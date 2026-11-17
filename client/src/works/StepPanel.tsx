// The Step of the item's window on a work's page: everything about the row of the work's history the owner chose. Of a
// run: how it ended or stands, where the work waits at it for the calls of tools in the words the work's state says
// that in everywhere, when it began and how long it took, what it took, what its rounds ran on and the sessions
// they ran in, with each new session it began and why; why it ran; the versions it started from, those new to it first;
// what it handed in, each version's content opened where it stands; each round, with what it was given (the reasons a
// return was sent back among them), what came back, what it ran on, took, was filled with, was told and returned; and
// its calls. Of a write of the owner's or the outside's: who wrote it, the block's version and value, whether it changed
// the block's meaning, the runs it ran and its content. Of another act: what was done, and when. With nothing chosen,
// that a row is to be chosen. A content is read only when the owner opens it.

import { Fragment, type ReactNode, useCallback } from 'react';

import { ListRow, PanelPart, Sign, StatusLine } from '../parts';
import type { CallRow, Doing, Handed, Round, RoundPage, Run, Step, ToldPage, WorkPage } from '../works-api';
import { CallsTook, Opener, Parts, RunMarkSign } from './marks';
import { type HistoryRow, type SessionStep, runMark } from './placing';
import { ContextKeys, DoingNow, Filled, Keys, MissedKeys, RanKeys, SystemPrompt, TookKeys } from './Round';
import type { RunMark } from './state-marks';
import { RoundTold } from './Told';
import {
  type Part,
  RUN_MARK_WORDS,
  askWords,
  callState,
  callWaitParts,
  callWords,
  callsWaited,
  causeRest,
  clock,
  duration,
  fullTime,
  givenWords,
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

/// What the Step reads the contents through, each read once.
export interface Reader {
  /// The content of the block's version of this number.
  version(block: string, number: number): Promise<string>;
  /// A round in full; kept once read only where the round had ended, since one under way has more to come.
  round(run: number, number: number, ended: boolean): Promise<RoundPage>;
  /// What a round was told and returned, as its carrier kept them; kept once read only where the round had ended.
  told(run: number, number: number, ended: boolean): Promise<ToldPage>;
  call(call: number): Promise<string | null>;
  /// The system prompt the line's session of this number, the line's first being none, began with; kept once read.
  system(line: string, session: number): Promise<string>;
  /// What the agent of a round was last seen doing, the latest last; read anew each time.
  doing(run: number, number: number): Promise<Doing[]>;
}

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

/// What a run said of a change it handed in: it is its word, which the engine takes for prose and not for a choice.
export function saidWords(handed: Handed): string {
  return handed.matters ? 'said to matter' : 'said not to matter';
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
      <span className="mono muted">call {row.call}</span>
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

/// One round: what it was given, what came back, when it began and ended, what it ran on, what it took, the context it
/// ended with, what was filled into its session and, while it is under way, what its agent is doing, read only while
/// the Step is `shown`; then the system prompt of its session, and what it was told and what it returned, as its
/// carrier kept them.
function RoundPart({ run, round, reader, shown }: { run: Run; round: Round; reader: Reader; shown: boolean }) {
  const doing = useCallback(() => reader.doing(run.run, round.number), [reader, run.run, round.number]);
  const session = round.session;
  const given = round.given;
  const back = round.back;
  const when =
    round.ended === null
      ? `began ${clock(round.began)}, under way`
      : `${clock(round.began)} to ${clock(round.ended)}, ${duration(round.ended - round.began)}`;
  const calls = given.kind === 'results' ? run.calls.filter((c) => given.calls.includes(c.call)) : [];
  return (
    <PanelPart
      label={
        <>
          Round {round.number} <span className="muted small">{when}</span>
        </>
      }
    >
      <dl className="pairs round">
        <dt>Given</dt>
        <dd>
          {given.kind === 'sentBack' ? (
            <>
              its return sent back:
              <ul className="reasons">
                {given.unmet.map((u, i) => (
                  <ListRow key={i}>{unmetWords(u)}</ListRow>
                ))}
              </ul>
            </>
          ) : (
            givenWords(given)
          )}
          {calls.length > 0 && (
            <ul>
              {calls.map((c) => (
                <ListRow key={c.call}>
                  <span className="mono muted">call {c.call}</span>
                  <span>{callWords(c.what)}</span>
                  <Opener
                    label="result"
                    load={async () =>
                      (await reader.round(run.run, round.number, round.ended !== null)).results.find((r) => r.call === c.call)?.result ?? ''
                    }
                  />
                </ListRow>
              ))}
            </ul>
          )}
        </dd>
        <dt>Back</dt>
        <dd>
          {back === null && <span className="muted">{round.ended === null ? 'nothing yet' : 'nothing: the run was abandoned'}</span>}
          {back?.kind === 'failed' && <span className="failed">nothing; the round failed: {back.why}</span>}
          {back?.kind === 'asked' && (
            <>
              asked for {back.asks.length === 0 ? 'nothing' : ''}
              <ul>
                {back.asks.map((a, i) => (
                  <ListRow key={i}>
                    <span>{askWords(a)}</span>
                    {a.kind === 'tool' ? (
                      <span className="mono call-input">{a.input}</span>
                    ) : (
                      <span className="question">{a.question}</span>
                    )}
                  </ListRow>
                ))}
              </ul>
            </>
          )}
          {back?.kind === 'handedIn' && (
            <>
              handed in {back.blocks.length === 0 ? 'nothing' : ''}
              <ul>
                {back.blocks.map((h, i) => (
                  <HandedRow
                    key={i}
                    handed={h}
                    load={async () => (await reader.round(run.run, round.number, round.ended !== null)).handed[i]?.content ?? ''}
                  />
                ))}
              </ul>
            </>
          )}
        </dd>
        {(session !== null || round.tries.length > 0) && (
          <>
            <dt>Ran</dt>
            <dd>
              <RanKeys round={round} />
            </dd>
          </>
        )}
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
        {round.tries.some((t) => t.filled.length > 0) && (
          <>
            <dt>Filled</dt>
            <dd>
              <Filled tries={round.tries} />
            </dd>
          </>
        )}
        {round.ended === null && session !== null && (
          <>
            <dt>Doing</dt>
            <dd>
              <DoingNow load={doing} shown={shown} />
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

/// How a run stands, as its Step's first line says it: its mark's words, and of a run under way whether it is between
/// rounds or its last round failed.
function standWords(run: Run, mark: RunMark): string {
  if (mark !== 'underWay') return capital(RUN_MARK_WORDS[mark]);
  return run.rounds.at(-1)?.back?.kind === 'failed' ? 'Its last round failed, to be given again' : 'Under way, between rounds';
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
  mark: RunMark;
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
  const calls = mark === 'calling' ? callsWaited(page.summary) : [];
  // What its rounds ran on and the sessions they ran in, each once, in the order its rounds first did.
  const ranOn = [
    ...new Map(
      run.rounds.flatMap((r) => {
        const ran = r.tries[r.tries.length - 1]?.ran;
        return ran ? [[`${ran.model} ${ran.level} ${ran.effort ?? ''}`, ran] as const] : [];
      }),
    ).values(),
  ];
  const sessions = [
    ...new Map(run.rounds.flatMap((r) => (r.session ? [[`${r.session.line} ${r.session.number}`, r.session] as const] : []))).values(),
  ];
  // The blocks it was told the change of first, then the rest it started from.
  const changed = new Set(run.because.flatMap((c) => (c.kind === 'changed' ? [c.block] : [])));
  const basis = [...run.basis].sort((a, b) => Number(changed.has(b.block)) - Number(changed.has(a.block)));
  // What it wrote before it finished, as a question it asked, apart from what it handed in as it finished.
  const before = page.blocks.flatMap((b) =>
    b.versions
      .filter((v) => v.by.kind === 'run' && v.by.run === run.run && v.at !== end?.at)
      .map((v) => ({ block: b.name, version: v })),
  );
  return (
    <>
      <div className="status-line">
        <RunMarkSign mark={mark} />
        <span className="status-word">
          {calls.length > 0 ? <Parts parts={callWaitParts(calls)} /> : standWords(run, mark)}
        </span>
        {calls.length > 0 && <CallsTook work={page.summary} />}
        <span className="mono">{run.cell}</span>
        {cell !== undefined && <span>{cell.title}</span>}
        <span className="mono muted">run {run.run}</span>
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
      {sessions.map((s) => (
        <StatusLine key={`${s.line} ${s.number}`} word="Session" dot={false}>
          <Keys keys={[['line', s.line], ['session', String(s.number)]]} />
        </StatusLine>
      ))}
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
      <PanelPart label="Started from" count={basis.length}>
        <ul>
          {basis.map((b) => (
            <ListRow key={b.block}>
              <VersionName block={b.block} number={b.version} />
              {changed.has(b.block) && <span className="changed-word">new to it</span>}
              {b.version === null ? (
                <span className="muted">no version yet</span>
              ) : (
                <Opener label="content" load={() => reader.version(b.block, b.version as number)} />
              )}
            </ListRow>
          ))}
        </ul>
      </PanelPart>
      <PanelPart label="Handed in" last>
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
      {run.rounds.map((round) => (
        <RoundPart key={round.number} run={run} round={round} reader={reader} shown={shown} />
      ))}
      <PanelPart label="Calls" count={run.calls.length} last>
        {run.calls.length === 0 ? (
          <p className="muted">It made none</p>
        ) : (
          <ul>
            {run.calls.map((c) => (
              <CallLine key={c.call} row={c} reader={reader} abandoned={end?.kind === 'abandoned'} />
            ))}
          </ul>
        )}
      </PanelPart>
    </>
  );
}

/// The runs a write ran, each by its cell and a link that chooses its row; none yet where it ran none.
function RanRuns({ row, page, onChoose }: { row: HistoryRow; page: WorkPage; onChoose: (step: number) => void }) {
  if (row.ran.length === 0) return <p className="muted">None yet</p>;
  return (
    <ul>
      {row.ran.map((r) => {
        const index = page.steps.findIndex((s) => s.kind === 'run' && s.run === r.run);
        const title = page.flow.cells.find((c) => c.id === r.cell)?.title;
        return (
          <ListRow key={r.run}>
            <span className="mono">{r.cell}</span>
            {title !== undefined && <span>{title}</span>}
            {index >= 0 && (
              <button type="button" className="link mono" onClick={() => onChoose(index)}>
                run {r.run}
              </button>
            )}
          </ListRow>
        );
      })}
    </ul>
  );
}

/// A write of the owner's or the outside's.
function WriteStep({ step, row, page, reader, onChoose }: {
  step: Step & { kind: 'wrote' };
  row: HistoryRow;
  page: WorkPage;
  reader: Reader;
  onChoose: (step: number) => void;
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
      <PanelPart label="Which cells it ran" count={row.ran.length} last>
        <RanRuns row={row} page={page} onChoose={onChoose} />
      </PanelPart>
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

/// The Step: the row chosen, or, with none, that a row is to be chosen. `shown` is whether the Step is in view, which
/// what a round's agent is doing is read only while it is.
export function StepTab({ page, row, sessions, reader, shown, onChoose }: {
  page: WorkPage;
  row: HistoryRow | undefined;
  /// The new sessions each run began, by the run's number.
  sessions: Map<number, SessionStep[]>;
  reader: Reader;
  shown: boolean;
  onChoose: (step: number) => void;
}) {
  if (row === undefined) return <p className="muted">Choose a row of the history to see its step here</p>;
  const step = row.step;
  switch (step.kind) {
    case 'run':
      return (
        <RunStep
          run={step}
          mark={runMark(step, page.summary, page.blocks)}
          began={sessions.get(step.run) ?? []}
          page={page}
          reader={reader}
          shown={shown}
        />
      );
    case 'wrote':
      return <WriteStep step={step} row={row} page={page} reader={reader} onChoose={onChoose} />;
    case 'session':
      return null;
    default:
      return <ActStep step={step} />;
  }
}
