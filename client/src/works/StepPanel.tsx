// The Step of the item's window: everything about the row of the work's history it is given, the one the owner chose
// on the work's page, else the run the work stands at (`standingRun`). Of a run: how it ended or stands, where the
// work waits at it for the calls of tools in the words the work's state says that in everywhere, when it began and how
// long it took, what it took, what it ran on, with each new session it began and why; why it ran, in plain words; its
// inputs at the versions it began with, those new to it first; what it handed in, each version's content opened where
// it stands; and each time it ran, in order, a return sent back said where the next time begins: what came back (the
// calls it asked the engine for with their results, or what was sent back), what it took, and what its agent did, was
// told and returned, each time's own when only where there were several. No number the engine keeps a run, a round or a call by is shown. Of a write of the owner's that woke no cell: what it did, the block's version and value, whether it changed
// the block's meaning, and its content. Of another act: what was done, and when. With no row, that the work stands at
// no run. A content is read only when the owner opens it.

import { Fragment, type ReactNode, useCallback } from 'react';

import { StateMark, markName } from '../marks';
import { ListRow, PanelPart, Sign, StatusLine } from '../parts';
import type { CallRow, Following, Handed, Round, Run, Step, WorkPage } from '../works-api';
import { CallsTook, Opener, Parts } from './marks';
import { type HistoryRow, type SessionStep, runState } from './placing';
import { ContextKeys, DoneList, Keys, MissedKeys, SystemPrompt, TookKeys } from './Round';
import type { Reader } from './source';
import type { RunState } from './state-marks';
import { RoundTold } from './Told';
import {
  type Part,
  callState,
  callWaitParts,
  callWords,
  callsWaited,
  causeRest,
  clock,
  duration,
  fullTime,
  isOpen,
  isWayRound,
  plain,
  runTook,
  sentence,
  sessionWords,
  spentWords,
  unmetWords,
  writerWords,
} from './words';

const chars = (n: number) => `${n.toLocaleString('en')} chars`;

/// A text with its first letter a capital, as a status line's word begins.
const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

/// A line of pieces as a status line's word begins it.
const capitalised = (parts: Part[]): Part[] =>
  parts.map((p, i) => (i === 0 && typeof p === 'string' ? capital(p) : p));

/// A version of a block, named by its number among the block's versions: `plan v2`.
function VersionName({ block, number }: { block: string; number: number | null }) {
  return (
    <span className="mono">
      {block} <span className="muted">{number === null ? 'none' : `v${number}`}</span>
    </span>
  );
}

/// The version of the block written at this place, where there is one.
const versionAt = (page: WorkPage, block: string, at: number) =>
  page.blocks.find((b) => b.name === block)?.versions.find((v) => v.at === at);

/// What a run said of a change it handed in: whether it matters to the cells that read the block, its word, which the
/// engine takes for prose and not for a choice.
export function saidWords(handed: Handed): string {
  return handed.matters ? 'it said this matters to the cells that read it' : 'it said this changes nothing they read';
}

/// A block handed in: its name, its value, whether the run said the change matters, its length, and its content; and,
/// for a block a finished run wrote, whether that version changed the block's meaning.
function HandedRow({ handed, load, meaning }: { handed: Handed; load: () => Promise<string>; meaning?: ReactNode }) {
  return (
    <ListRow>
      <span className="mono">{handed.block}</span>
      {handed.value !== null && <span className="mono">= {handed.value}</span>}
      <span className="muted">{saidWords(handed)}</span>
      {meaning}
      <span className="muted">{chars(handed.chars)}</span>
      <Opener label="content" load={load} />
    </ListRow>
  );
}

/// A call: what it is, its input for a tool, when it was made and done, and its result.
function CallLine({ row, reader, abandoned }: { row: CallRow; reader: Reader; abandoned: boolean }) {
  const what = row.what;
  return (
    <ListRow>
      <span>{callWords(what)}</span>
      {what.kind === 'tool' && <span className="mono call-input">{what.input}</span>}
      {what.kind === 'question' && <span className="question">{what.question}</span>}
      <span className="muted small">
        {callState(row, abandoned)}
        {row.chars !== null ? `, ${chars(row.chars)}` : ''}
      </span>
      {row.done !== null && <Opener label="result" load={async () => (await reader.call(row.call)) ?? ''} />}
    </ListRow>
  );
}

/// The details of each time the cell was run within the run, when it was where there were several: where it is not the
/// first, why it ran again, its return sent back or its
/// last time having failed; what came back: the calls it asked the engine for, each with its result, or what it handed
/// in, listed where it was sent back, since what was kept is under Handed in; what it took, the context it ended with,
/// how it missed its session's cache, and what its agent did, followed or read only while the Step is `shown`; then the
/// system prompt of its session, and what it was told and what it returned, as its carrier kept them.
function RoundPart({ run, round, calls, first, timed, sentBack, failedBefore, reader, shown }: {
  run: Run;
  round: Round;
  /// The run ran its cell more than once, so that each time says when it was; once, the run's own time says it.
  timed: boolean;
  /// The calls it asked for, made after it began and before the next time did.
  calls: CallRow[];
  first: boolean;
  /// What it handed in was sent back, by the time after it.
  sentBack: boolean;
  /// The time before it failed.
  failedBefore: boolean;
  reader: Reader;
  shown: boolean;
}) {
  const doing = useCallback(() => reader.doing(run.run, round.number), [reader, run.run, round.number]);
  const follow = useCallback(
    (following: Following) => reader.follow(run.run, round.number, following),
    [reader, run.run, round.number],
  );
  const session = round.session;
  const given = round.given;
  const back = round.back;
  const when =
    round.ended === null
      ? `began ${clock(round.began)}, under way`
      : `${clock(round.began)} to ${clock(round.ended)}, ${duration(round.ended - round.began)}`;
  return (
    <PanelPart
      label={
        <>
          {first ? 'Details' : 'Ran again'}
          {timed && <span className="muted small"> {when}</span>}
        </>
      }
    >
      <dl className="pairs round">
        {given.kind === 'sentBack' && (
          <>
            <dt>Sent back</dt>
            <dd>
              <ul className="reasons">
                {given.unmet.map((u, i) => (
                  <ListRow key={i}>{unmetWords(u)}</ListRow>
                ))}
              </ul>
            </dd>
          </>
        )}
        {!first && given.kind !== 'sentBack' && (failedBefore || given.kind === 'results') && (
          <>
            <dt>Why</dt>
            <dd>{failedBefore ? 'the time before failed' : 'the results of the calls it asked for came back'}</dd>
          </>
        )}
        <dt>Back</dt>
        <dd>
          {back === null && <span className="muted">{round.ended === null ? 'nothing yet' : 'nothing: the run was abandoned'}</span>}
          {back?.kind === 'failed' && <span className="failed">nothing; it failed: {back.why}</span>}
          {back?.kind === 'asked' && (
            <>
              asked the engine for{' '}
              {back.asks.length === 0 ? 'nothing' : back.asks.length === 1 ? '1 call' : `${back.asks.length} calls`}
              {calls.length > 0 && (
                <ul>
                  {calls.map((c) => (
                    <CallLine key={c.call} row={c} reader={reader} abandoned={run.end?.kind === 'abandoned'} />
                  ))}
                </ul>
              )}
            </>
          )}
          {back?.kind === 'handedIn' && (
            <>
              handed in {back.blocks.length === 0 ? 'nothing' : sentBack ? 'this, which was sent back' : 'what is under Handed in'}
              {sentBack && (
                <ul>
                  {back.blocks.map((h, i) => (
                    <HandedRow
                      key={i}
                      handed={h}
                      load={async () =>
                        (await reader.round(run.run, round.number, round.ended !== null)).handed[i]?.content ?? ''
                      }
                    />
                  ))}
                </ul>
              )}
            </>
          )}
        </dd>
        {round.spent && (
          <>
            <dt>Took</dt>
            <dd>
              <TookKeys spent={round.spent} estimated={round.estimated} />
            </dd>
          </>
        )}
        {round.context !== null && (
          <>
            <dt>Context</dt>
            <dd>
              <ContextKeys context={round.context} compactions={round.compactions} />
            </dd>
          </>
        )}
        {round.missed && (
          <>
            <dt>Missed</dt>
            <dd>
              <MissedKeys missed={round.missed} />
            </dd>
          </>
        )}
        {session !== null && (
          <>
            <dt>{round.ended === null ? 'Doing' : 'Did'}</dt>
            <dd>
              <DoneList underWay={round.ended === null} follow={follow} load={doing} shown={shown} />
            </dd>
          </>
        )}
      </dl>
      {session !== null && <SystemPrompt load={() => reader.system(session.line, session.number)} />}
      <RoundTold load={() => reader.told(run.run, round.number, round.ended !== null)} ended={round.ended} />
    </PanelPart>
  );
}

/// What the engine made of a version a run wrote: whether it changed the block's meaning. A choice changes it when its
/// value does, whatever the run said; prose, when the run said the change matters. A first version handed in as
/// changing nothing has no meaning: the block has said nothing yet.
function MeaningWord({ page, block, at }: { page: WorkPage; block: string; at: number }) {
  const version = versionAt(page, block, at);
  if (!version) return null;
  if (version.meaning === null) return <span className="muted">said nothing yet</span>;
  return version.meaning === version.number ? (
    <span className="changed-word">changed its meaning</span>
  ) : (
    <span className="muted">kept the meaning of v{version.meaning}</span>
  );
}

/// How a run stands, as its Step's first line says it: its mark's name, and of a run that failed whether its last round
/// failed or what it handed in was sent back, each to be given again.
function standWords(run: Run, mark: RunState): string {
  if (mark !== 'failed') return capital(markName(mark));
  const why = run.rounds.at(-1)?.back?.kind === 'handedIn' ? 'its return was sent back' : 'its last round failed';
  return `${capital(markName(mark))}: ${why}, to be given again`;
}

/// A new session a run began, and why: begun anew, begun again under its id, or compacted.
function SessionLine({ step }: { step: SessionStep }) {
  const word = step.why.kind === 'compacted' ? 'Compacted' : 'New session';
  return (
    <StatusLine word={word} dot={false}>
      <span className="line-name">{step.line}</span>, {sessionWords(step.why)}
    </StatusLine>
  );
}

/// Everything about a run.
function RunStep({ run, mark, began, page, reader, shown }: {
  run: Run;
  mark: RunState;
  /// The new sessions it began.
  began: SessionStep[];
  page: WorkPage;
  reader: Reader;
  shown: boolean;
}) {
  const cell = page.flow.cells.find((c) => c.id === run.cell);
  const now = Date.now();
  const end = run.end;
  const wayRound = isWayRound(run);
  // The calls of tools the work waits at this run for, where it does: the run then stands as the work does.
  const calls = mark === 'waiting' ? callsWaited(page.summary) : [];
  // What its rounds ran on and the sessions they ran in, each once, in the order its rounds first did.
  const ranOn = [
    ...new Map(
      run.rounds.flatMap((r) => {
        const ran = r.tries[r.tries.length - 1]?.ran;
        return ran ? [[`${ran.model} ${ran.level} ${ran.effort ?? ''}`, ran] as const] : [];
      }),
    ).values(),
  ];
  // Its inputs, the blocks it is given, watches or reads, at the versions it began with: those it was told the change
  // of first; none that had no version yet, nor what it may only write, which Handed in shows.
  const changed = new Set(run.because.flatMap((c) => (c.kind === 'changed' ? [c.block] : [])));
  const taken = new Set([
    ...(cell?.given ?? []).map((g) => g.block),
    ...[...(cell?.watches ?? []), ...(cell?.reads ?? [])].flatMap((w) => w.blocks),
  ]);
  const inputs = run.basis
    .filter((b) => b.version !== null && taken.has(b.block))
    .sort((a, b) => Number(changed.has(b.block)) - Number(changed.has(a.block)));
  // What it wrote before it finished, as a question it asked, apart from what it handed in as it finished.
  const before = page.blocks.flatMap((b) =>
    b.versions
      .filter((v) => v.by.kind === 'run' && v.by.run === run.run && v.at !== end?.at)
      .map((v) => ({ block: b.name, version: v })),
  );
  return (
    <>
      <div className="status-line">
        <StateMark state={mark} />
        <span className="status-word">
          {calls.length > 0 ? <Parts parts={callWaitParts(calls)} /> : standWords(run, mark)}
        </span>
        {calls.length > 0 && <CallsTook work={page.summary} />}
        <span className="mono">{run.cell}</span>
        {cell !== undefined && <span>{cell.title}</span>}
      </div>
      <StatusLine word="When" dot={false}>
        began {clock(run.time)}, {isOpen(run) ? 'open for' : 'took'} {duration(runTook(run, now))}
      </StatusLine>
      <StatusLine word="Took" dot={false}>
        {spentWords(run.spent)}
      </StatusLine>
      {ranOn.length > 0 && (
        <StatusLine word="Ran on" dot={false}>
          {ranOn.map((r, i) => (
            <Fragment key={i}>
              {i > 0 && '; '}
              <Keys keys={[['model', r.model], ['level', r.level], ...(r.effort ? [['effort', r.effort] as [string, string]] : [])]} />
            </Fragment>
          ))}
        </StatusLine>
      )}
      {began.map((s) => (
        <SessionLine key={s.at} step={s} />
      ))}
      <PanelPart label="Why it ran" count={run.because.length}>
        <ul>
          {run.because.map((c, i) => (
            <ListRow key={i}>
              {c.kind === 'changed' && <span className="mono">{c.block}</span>}
              <span className={c.kind === 'changed' && c.behind && wayRound ? 'way-back' : 'muted'}>
                {causeRest(c, wayRound)}
              </span>
            </ListRow>
          ))}
        </ul>
      </PanelPart>
      <PanelPart label="Inputs" count={inputs.length}>
        {inputs.length === 0 ? (
          <p className="muted">None had a version yet</p>
        ) : (
          <ul>
            {inputs.map((b) => (
              <ListRow key={b.block}>
                <VersionName block={b.block} number={b.version} />
                {changed.has(b.block) && <span className="changed-word">new to it</span>}
                <Opener label="content" load={() => reader.version(b.block, b.version as number)} />
              </ListRow>
            ))}
          </ul>
        )}
      </PanelPart>
      <PanelPart label="Handed in">
        {end === null && <p className="muted">Nothing yet</p>}
        {end?.kind === 'abandoned' && <p>Abandoned {clock(end.time)}; it wrote nothing</p>}
        {end?.kind === 'finished' && (
          <>
            <p>
              Finished {clock(end.time)}, and wrote{' '}
              {end.writes.length === 0 ? 'nothing' : `${end.writes.length} block${end.writes.length === 1 ? '' : 's'}`}
            </p>
            <ul>
              {end.writes.map((w) => {
                // The version the run wrote, read by its number among the block's versions.
                const number = versionAt(page, w.block, end.at)?.number;
                return (
                  <HandedRow
                    key={w.block}
                    handed={w}
                    load={() =>
                      number === undefined
                        ? Promise.reject(new Error(`The work shows no version of ${w.block} its run wrote`))
                        : reader.version(w.block, number)
                    }
                    meaning={<MeaningWord page={page} block={w.block} at={end.at} />}
                  />
                );
              })}
            </ul>
          </>
        )}
        {before.length > 0 && (
          <ul>
            {before.map(({ block, version }) => (
              <ListRow key={`${block} ${version.number}`}>
                <VersionName block={block} number={version.number} />
                {version.value !== null && <span className="mono">= {version.value}</span>}
                <span className="muted small">{clock(version.time)}</span>
                <span className="muted">{chars(version.chars)}</span>
                <Opener label="content" load={() => reader.version(block, version.number)} />
              </ListRow>
            ))}
          </ul>
        )}
      </PanelPart>
      {run.rounds.map((round, i) => (
        <RoundPart
          key={round.number}
          run={run}
          round={round}
          calls={run.calls.filter((c) => c.made >= round.began && c.made < (run.rounds[i + 1]?.began ?? Infinity))}
          first={i === 0}
          timed={run.rounds.length > 1}
          sentBack={run.rounds[i + 1]?.given.kind === 'sentBack'}
          failedBefore={run.rounds[i - 1]?.back?.kind === 'failed'}
          reader={reader}
          shown={shown}
        />
      ))}
    </>
  );
}

/// A write of the owner's that woke no cell: one that woke a cell is named by the run it woke.
function WriteStep({ step, row, page, reader }: {
  step: Step & { kind: 'wrote' };
  row: HistoryRow;
  page: WorkPage;
  reader: Reader;
}) {
  const version = versionAt(page, step.block, step.at);
  const who = writerWords(step.by);
  return (
    <>
      <div className="status-line">
        {step.by.kind !== 'run' && <Sign name={step.by.kind === 'owner' ? 'people' : 'outside'} words={who} />}
        <span className="status-word">
          <Parts parts={capitalised(row.did)} />
        </span>
        <span className="muted">by {who}</span>
      </div>
      <StatusLine word="Version" dot={false}>
        <VersionName block={step.block} number={version?.number ?? null} />
        {step.value !== null && <span className="mono"> = {step.value}</span>}
      </StatusLine>
      {version && version.meaning === null ? (
        <StatusLine word="Said nothing yet" />
      ) : (
        <StatusLine tone={step.changed ? 'changed' : 'quiet'} word={step.changed ? 'Changed its meaning' : 'Same meaning'}>
          {!step.changed && version && version.meaning !== null ? `as at v${version.meaning}` : undefined}
        </StatusLine>
      )}
      <StatusLine word="When" dot={false}>
        {fullTime(step.time)}, {chars(step.chars)}
      </StatusLine>
      <PanelPart label="Content">
        <Opener
          label="content"
          load={() =>
            version === undefined
              ? Promise.reject(new Error(`The work shows no version of ${step.block} written here`))
              : reader.version(step.block, version.number)
          }
        />
      </PanelPart>
    </>
  );
}

const ACT_WORDS = {
  letGo: 'Let go on',
  paused: 'Paused',
  resumed: 'Resumed',
  cancelled: 'Cancelled',
  reopened: 'Reopened',
} as const;

/// An act on the work: a let-go, a pause, a let-on, a cancel or a reopening of the owner's, or a pause of the engine's.
function ActStep({ step }: { step: Step & { kind: keyof typeof ACT_WORDS } }) {
  const what = {
    letGo: step.kind === 'letGo' ? `The owner let ${step.cell} go on past its limit of ways round, and its count started anew` : '',
    paused:
      step.kind === 'paused' && step.by === 'engine'
        ? `The engine paused the work. Only a note to a cell is answered until the owner lets it on. ${plain(sentence(step.why ?? 'It gave no reason'))}`
        : 'The owner paused the work. Only a note to a cell is answered, and a round under way goes on.',
    resumed: 'The work was let on: its rounds begin again',
    cancelled: `The work's item was cancelled: its round under way was stopped, its calls withdrawn, and its runs abandoned. ${
      step.kind === 'cancelled' && step.why ? sentence(step.why) : ''
    }`,
    reopened:
      step.kind === 'reopened'
        ? `The work was reopened, paused until it is let on, when it makes its place again and goes on from its records. ${sentence(step.why)}`
        : '',
  }[step.kind];
  return (
    <>
      <StatusLine word={step.kind === 'paused' && step.by === 'engine' ? 'Paused by the engine' : ACT_WORDS[step.kind]}>
        {fullTime(step.time)}
      </StatusLine>
      <PanelPart label="What was done">
        <p>{what.trim()}</p>
      </PanelPart>
    </>
  );
}

/// The Step: the row of the work's history it is given, or, with none, that the work stands at no run. `shown` is
/// whether the Step is in view, which what a round's agent does is followed or read only while it is.
export function StepTab({ page, row, sessions, reader, shown }: {
  page: WorkPage;
  row: HistoryRow | undefined;
  /// The new sessions each run began, by the run's number.
  sessions: Map<number, SessionStep[]>;
  reader: Reader;
  shown: boolean;
}) {
  if (row === undefined) return <p className="muted">The work stands at no run</p>;
  const step = row.step;
  switch (step.kind) {
    case 'run':
      // Drawn anew for another run, so that nothing opened or followed of one run's rounds stands for another's.
      return (
        <RunStep
          key={step.run}
          run={step}
          mark={runState(step, page.summary)}
          began={sessions.get(step.run) ?? []}
          page={page}
          reader={reader}
          shown={shown}
        />
      );
    case 'wrote':
      return <WriteStep step={step} row={row} page={page} reader={reader} />;
    case 'session':
      return null;
    default:
      return <ActStep step={step} />;
  }
}
