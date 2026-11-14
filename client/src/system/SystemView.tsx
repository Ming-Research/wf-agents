// The System view: how the whole system stands, in parts, each for what the owner does with it. Now says whether the
// owner needs to step in: the engine, its slots, the wait every work shares, the copies of the database, the works by
// where they stand, and the rounds under way. Produced says what got done, Spent where the money and the time went,
// and Gone wrong what was wasted or failed, each over the span the owner picks in the view's line: the week under way,
// the four weeks to its end, or all time. Each part sums up first, in numbers that stand alone; a number that counts a
// list unfolds it under the numbers. Each thing is said once: the engine's pause and its slots in Now, and the rounds
// under way in their list, not among the numbers of the works.

import { useCallback, useEffect, useMemo, useState } from 'react';

import { PanelPart, Switch } from '../parts';
import { hashOf, type Period } from '../route';
import type { Copies, Engine, Item, Items, Spent } from '../works-api';
import { copiesTone, copiesWords } from '../works/Copies';
import { NowLine } from '../works/marks';
import { useChanges, useItems, useReading, useSource } from '../works/source';
import {
  allTokens,
  clock,
  dollars,
  duration,
  failureWords,
  fullTime,
  outageWords,
  tokens,
  tokensWords,
} from '../works/words';
import { type RanOn, type System, useSystemSource } from './api';
import { ItemName, Tile, Tiles, ViewTable, WorkLink } from './parts';
import {
  cacheShare,
  mergeAverages,
  percent,
  STANDING_WORDS,
  STANDINGS,
  type Standing,
  standingOf,
  summed,
  nextWeekStart,
  weekStarts,
} from './standing';

const SPANS: { value: Period; name: string; title: string }[] = [
  { value: 'week', name: 'This week', title: "From this week's Monday" },
  { value: '4w', name: '4 weeks', title: 'From the Monday three weeks before this one' },
  { value: 'all', name: 'All time', title: 'Everything the engine recorded' },
];

/// How many rows of a long list are drawn before the rest are counted.
const ROWS = 50;

export function SystemView({ period }: { period: Period }) {
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
          <Switch<Period>
            label="The span the numbers count over"
            value={period}
            onChange={(p) => {
              window.location.hash = hashOf({ view: 'system', period: p });
            }}
            choices={SPANS}
          />
        </span>
      </div>
      {[items.failed, engine.failed, span.failed].some((f) => f !== null) && (
        <p className="read-failed" role="alert">
          {[
            items.failed && `The works could not be read: ${items.failed}.`,
            engine.failed && `The engine could not be read: ${engine.failed}.`,
            span.failed && `The span could not be read: ${span.failed}.`,
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
          <NowPart items={items.value} engine={engine.value} copies={copies.value} copiesFailed={copies.failed} />
        )}
        {span.value === null ? (
          span.failed === null && <p className="message muted">Reading the span</p>
        ) : (
          <>
            <Produced s={span.value} items={byNumber} />
            <SpentPart s={span.value} items={byNumber} />
            <GoneWrong s={span.value} items={byNumber} />
          </>
        )}
      </main>
    </>
  );
}

/// Whether the owner needs to step in: the engine, the wait every work shares, the copies; the works by where they
/// stand, each count unfolding its works; and the rounds under way.
function NowPart({ items, engine, copies, copiesFailed }: {
  items: Items;
  engine: Engine | null;
  copies: Copies | null;
  copiesFailed: string | null;
}) {
  const [open, setOpen] = useState<Standing | null>(null);
  // The works whose round is under way are in the list of the rounds under way, not among the numbers.
  const by = STANDINGS.filter((standing) => standing !== 'running').map((standing) => ({
    standing,
    list: items.items.filter((item) => standingOf(item.work) === standing),
  }));
  const running = items.items.filter((item) => item.work.now.kind === 'round');
  const opened = by.find((b) => b.standing === open);
  return (
    <PanelPart label="Now">
      <dl className="pairs view-pairs">
        <dt>Engine</dt>
        <dd>
          {engine === null
            ? 'not given'
            : engine.paused === null
              ? 'Every work goes on'
              : `Every work paused since ${clock(engine.paused.since)}, by the ${engine.paused.by}: no work begins a round but to answer a note`}
        </dd>
        <dt title="Rounds of agents under way, of how many may be at once: agents.slots">Slots</dt>
        <dd>{engine === null ? 'not given' : `${engine.taken} taken of ${engine.slots}`}</dd>
        <dt>The wait every work shares</dt>
        <dd className={items.outage ? 'failed' : undefined}>{items.outage ? outageWords(items.outage) : 'none'}</dd>
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
      </dl>
      <Tiles>
        {by.map(({ standing, list }) => (
          <Tile
            key={standing}
            value={list.length}
            label={STANDING_WORDS[standing].name}
            title={STANDING_WORDS[standing].title}
            open={open === standing}
            onOpen={list.length > 0 ? () => setOpen(open === standing ? null : standing) : undefined}
          />
        ))}
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
                <NowLine now={item.work.now} paused={item.work.paused} />
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
        <ViewTable heads={['Work', 'Item', 'Cell', 'Round', 'Since']}>
          {running.map((item) => (
            <RunningRow key={item.work.number} item={item} />
          ))}
        </ViewTable>
      )}
    </PanelPart>
  );
}

function RunningRow({ item }: { item: Item }) {
  const now = item.work.now;
  if (now.kind !== 'round') return null;
  return (
    <tr>
      <td>
        <WorkLink work={item.work.number} />
      </td>
      <td className="view-cell-wide">
        <ItemName item={item.number} title={item.title} />
      </td>
      <td className="mono">{now.cell}</td>
      <td className="mono">
        run {now.run}, round {now.round}
      </td>
      <td title={fullTime(now.since)}>{clock(now.since)}</td>
    </tr>
  );
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
        <Tile value={avg === null ? 'none' : dollars(avg.usdMicros)} label="A merge's cost, on average" />
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

/// Where the span's money and time went: in all, week by week over more than one week, by cell, by model, and by
/// work, the costliest first.
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
                <td className="mono">{ranOnWords(m.on)}</td>
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
        <ViewTable heads={['Began', 'Work', 'Item', 'Cell', 'Round', 'Cost']} numbers={[5]}>
          {s.sentBack.slice(0, ROWS).map((r) => (
            <tr key={`${r.work}/${r.run}/${r.round}`}>
              <td title={fullTime(r.began)}>{clock(r.began)}</td>
              <td>
                <WorkLink work={r.work} />
              </td>
              <td className="view-cell-wide">
                <ItemName item={r.item} title={items.get(r.item)?.title} />
              </td>
              <td className="mono">{r.cell}</td>
              <td className="mono">
                run {r.run}, round {r.round}
              </td>
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
          <ViewTable heads={['Ended', 'Work', 'Item', 'Cell', 'Round', 'Cause', 'Why']}>
            {s.failed.slice(0, ROWS).map((f) => (
              <tr key={`${f.work}/${f.run}/${f.round}`}>
                <td title={fullTime(f.ended)}>{clock(f.ended)}</td>
                <td>
                  <WorkLink work={f.work} />
                </td>
                <td className="view-cell-wide">
                  <ItemName item={f.item} title={items.get(f.item)?.title} />
                </td>
                <td className="mono">{f.cell}</td>
                <td className="mono">
                  run {f.run}, round {f.round}
                </td>
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
