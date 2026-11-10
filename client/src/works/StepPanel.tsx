// The panel docked at the right of a work's history: everything about the step chosen (a run: why it ran, what it
// started from, each round, its calls and how it ended; a write: the block, who, its value, whether it changed the
// meaning, and its content; an act of the owner's), or the cell chosen (how it stands, why it would run, what its
// record knows of each block it reads), or, with nothing chosen, every block that has versions, each version with its
// content. A content is read only when the owner opens it.

import type { ReactNode } from 'react';

import type { Flow } from '../api';
import { ListRow, Panel, PanelPart, StatusLine, type Tone } from '../parts';
import type { Fold } from '../Side';
import type { Block, CallRow, Handed, Round, RoundPage, Run, Step, WorkPage } from '../works-api';
import type { Chosen } from './History';
import { findRun } from './placing';
import { Opener, StateMark, cellNowWords } from './marks';
import {
  RUN_WORDS,
  STATE_WORDS,
  type RunState,
  askWords,
  callState,
  callWords,
  causeRest,
  clock,
  dollars,
  duration,
  fullTime,
  givenWords,
  isWayRound,
  runState,
  runTook,
  spentWords,
  tokensWords,
  unmetWords,
  writerWords,
} from './words';

/// What the panels read the contents through, each read once.
export interface Reader {
  version(block: string, at: number): Promise<string>;
  round(run: number, number: number): Promise<RoundPage>;
  call(call: number): Promise<string | null>;
}

/// What every panel of a work is given.
export interface PanelProps {
  page: WorkPage;
  reader: Reader;
  fold: Fold;
  foot: ReactNode;
  onChoose: (chosen: Chosen) => void;
}

const RUN_TONES: Record<RunState, Tone> = { finished: 'added', abandoned: 'removed', running: 'running', resting: 'quiet' };

const chars = (n: number) => `${n.toLocaleString('en')} chars`;

/// A version of a block, named by its place: `plan@12`.
function VersionName({ block, at }: { block: string; at: number | null }) {
  return (
    <span className="mono">
      {block}
      <span className="muted">@{at ?? 'none'}</span>
    </span>
  );
}

/// A block handed in: its name, its value, whether the run said the change matters, its length, and its content.
function HandedRow({ handed, load }: { handed: Handed; load: () => Promise<string> }) {
  return (
    <ListRow>
      <span className="mono">{handed.block}</span>
      {handed.value !== null && <span className="mono">= {handed.value}</span>}
      <span className="muted">{handed.matters ? 'the change matters' : 'the change does not matter'}</span>
      <span className="muted">{chars(handed.chars)}</span>
      <Opener label="content" load={load} />
    </ListRow>
  );
}

/// A call: what it is, its input for a tool, when it was made and done, and its result.
function CallLine({ row, reader }: { row: CallRow; reader: Reader }) {
  const what = row.what;
  return (
    <ListRow>
      <span className="mono muted">call {row.call}</span>
      <span>{callWords(what)}</span>
      {what.kind === 'tool' && <span className="mono call-input">{what.input}</span>}
      {what.kind === 'question' && <span className="question">{what.question}</span>}
      <span className="muted small">
        {callState(row)}
        {row.chars !== null ? `, ${chars(row.chars)}` : ''}
      </span>
      {row.done !== null && <Opener label="result" load={async () => (await reader.call(row.call)) ?? ''} />}
    </ListRow>
  );
}

/// One round: what it was given, what came back, when it began and ended, and what it took.
function RoundPart({ run, round, reader }: { run: Run; round: Round; reader: Reader }) {
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
                      (await reader.round(run.run, round.number)).results.find((r) => r.call === c.call)?.result ?? ''
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
                    load={async () => (await reader.round(run.run, round.number)).handed[i]?.content ?? ''}
                  />
                ))}
              </ul>
            </>
          )}
        </dd>
        {round.spent && (
          <>
            <dt>Took</dt>
            <dd>
              {dollars(round.spent.usdMicros)}, {tokensWords(round.spent)}
            </dd>
          </>
        )}
      </dl>
    </PanelPart>
  );
}

/// Everything about a run.
export function RunPanel({ run, page, reader, fold, foot }: PanelProps & { run: Run }) {
  const state = runState(run);
  const cell = page.flow.cells.find((c) => c.id === run.cell);
  const now = Date.now();
  const end = run.end;
  return (
    <Panel
      label={`Run ${run.run} of ${run.cell}`}
      id={`run ${run.run}`}
      title={cell?.title ?? run.cell}
      belongs={<span className="mono">{run.cell}</span>}
      lines={
        <>
          <StatusLine tone={RUN_TONES[state]} word={RUN_WORDS[state].replace(/^./, (c) => c.toUpperCase())}>
            began {clock(run.time)}, {state === 'running' || state === 'resting' ? 'open for' : 'took'}{' '}
            {duration(runTook(run, now))}
          </StatusLine>
          <StatusLine word="Took">{spentWords(run.spent)}</StatusLine>
          {isWayRound(run) && (
            <StatusLine tone="back" word="Way round">
              each of its reasons is a block a cell after it changed
            </StatusLine>
          )}
        </>
      }
      foot={foot}
      {...fold}
    >
      <PanelPart label="Why it ran" count={run.because.length}>
        <ul>
          {run.because.map((c, i) => (
            <ListRow key={i}>
              {c.kind === 'changed' && <span className="mono">{c.block}</span>}
              <span className={c.kind === 'changed' && c.behind ? 'way-back' : 'muted'}>{causeRest(c)}</span>
            </ListRow>
          ))}
        </ul>
      </PanelPart>
      <PanelPart label="Started from" count={run.basis.length} last>
        <ul>
          {run.basis.map((b) => (
            <ListRow key={b.block}>
              <VersionName block={b.block} at={b.version} />
              {b.version === null ? (
                <span className="muted">no version yet</span>
              ) : (
                <Opener label="content" load={() => reader.version(b.block, b.version as number)} />
              )}
            </ListRow>
          ))}
        </ul>
      </PanelPart>
      {run.rounds.map((round) => (
        <RoundPart key={round.number} run={run} round={round} reader={reader} />
      ))}
      <PanelPart label="Calls" count={run.calls.length} last>
        {run.calls.length === 0 ? (
          <p className="muted">It made none</p>
        ) : (
          <ul>
            {run.calls.map((c) => (
              <CallLine key={c.call} row={c} reader={reader} />
            ))}
          </ul>
        )}
      </PanelPart>
      <PanelPart label="How it ended">
        {end === null && (
          <p className="muted">
            {state === 'running'
              ? 'It has not: a round of it is under way'
              : `It has not: it is at rest${run.calls.some((c) => c.done === null) ? ', at a call with no result yet' : ''}`}
          </p>
        )}
        {end?.kind === 'abandoned' && (
          <p>
            Abandoned at <span className="mono">@{end.at}</span>, {clock(end.time)}; it wrote nothing
          </p>
        )}
        {end?.kind === 'finished' && (
          <>
            <p>
              Finished at <span className="mono">@{end.at}</span>, {clock(end.time)}, and wrote{' '}
              {end.writes.length === 0 ? 'nothing' : `${end.writes.length} block${end.writes.length === 1 ? '' : 's'}`}
            </p>
            <ul>
              {end.writes.map((w) => (
                <HandedRow key={w.block} handed={w} load={() => reader.version(w.block, end.at)} />
              ))}
            </ul>
          </>
        )}
      </PanelPart>
    </Panel>
  );
}

/// The cells that read a block, and those whose condition reads it, in words.
export function readersWords(flow: Flow, block: string): string {
  const b = flow.blocks.find((x) => x.name === block);
  if (!b) return 'No cell of the flow reads it';
  const parts = [
    ...(b.readBy.cells.length > 0 ? [`Read by ${b.readBy.cells.join(', ')}`] : []),
    ...(b.readBy.conditions.length > 0 ? [`decides whether ${b.readBy.conditions.join(', ')} runs`] : []),
  ];
  return parts.length > 0 ? parts.join('; ') : 'No cell reads it';
}

/// A write by a person, the outside or a run, as a step of its own.
export function WritePanel({ step, page, reader, fold, foot }: PanelProps & { step: Step & { kind: 'wrote' } }) {
  const version = page.blocks.find((b) => b.name === step.block)?.versions.find((v) => v.at === step.at);
  return (
    <Panel
      label={`Write of ${step.block}`}
      id={`@${step.at}`}
      title={<span className="mono">{step.block}</span>}
      belongs={`written by ${writerWords(step.by)}`}
      lines={
        <>
          <StatusLine tone={step.changed ? 'changed' : 'quiet'} word={step.changed ? 'Changed its meaning' : 'Same meaning'}>
            {step.changed
              ? 'the cells that read it run again for it'
              : version
                ? `as at @${version.meaning}`
                : undefined}
          </StatusLine>
          <StatusLine word="Written">
            {fullTime(step.time)}, {chars(step.chars)}
          </StatusLine>
          {step.value !== null && (
            <StatusLine word="Value">
              <span className="mono">{step.value}</span>
            </StatusLine>
          )}
        </>
      }
      foot={foot}
      {...fold}
    >
      <PanelPart label="Who reads it" last>
        <p className="muted">{readersWords(page.flow, step.block)}</p>
      </PanelPart>
      <PanelPart label="Content">
        <Opener label="content" load={() => reader.version(step.block, step.at)} />
      </PanelPart>
    </Panel>
  );
}

const ACT_WORDS = {
  letGo: 'Let go on',
  paused: 'Paused',
  resumed: 'Let on',
  cancelled: 'Cancelled',
} as const;

/// An act of the owner's on the work: a let-go, a pause, a let-on or a cancel.
export function ActPanel({ step, fold, foot }: PanelProps & { step: Step & { kind: keyof typeof ACT_WORDS } }) {
  const what = {
    letGo: step.kind === 'letGo' ? `The owner let ${step.cell} go on past its limit of ways round, and its count started anew` : '',
    paused: 'The owner paused the work: no round of it begins, and a round under way goes on',
    resumed: 'The owner let the work on: its rounds begin again',
    cancelled: 'The owner cancelled the work: its round under way was stopped, its calls withdrawn, and its runs abandoned',
  }[step.kind];
  return (
    <Panel
      label={ACT_WORDS[step.kind]}
      id={`@${step.at}`}
      title={ACT_WORDS[step.kind]}
      belongs={step.kind === 'letGo' ? <span className="mono">{step.cell}</span> : 'the work'}
      lines={<StatusLine word="When">{fullTime(step.time)}</StatusLine>}
      foot={foot}
      {...fold}
    >
      <p>{what}</p>
    </Panel>
  );
}

/// Everything about a cell of the work: how it stands, why it would run, and what its record knows.
export function WorkCellPanel({ cell, page, fold, foot, onChoose }: PanelProps & { cell: string }) {
  const flowCell = page.flow.cells.find((c) => c.id === cell);
  const now = page.summary.cells.find((c) => c.cell === cell);
  const detail = page.cells.find((c) => c.cell === cell);
  const runStep = (run: number | null) => (run === null ? -1 : page.steps.findIndex((s) => s.kind === 'run' && s.run === run));
  const openRun = (run: number | null, words: string) => {
    const index = runStep(run);
    return index < 0 ? null : (
      <button type="button" className="link" onClick={() => onChoose({ step: index })}>
        {words} {run}
      </button>
    );
  };
  return (
    <Panel
      label={`Cell ${cell}`}
      id={cell}
      title={flowCell?.title ?? cell}
      belongs={flowCell?.kind === 'agent' ? <span className="line-name">{flowCell.line}</span> : 'engine'}
      lines={
        now && (
          <>
            <StatusLine word={<><StateMark state={now.state} /> {STATE_WORDS[now.state].replace(/^./, (c) => c.toUpperCase())}</>}>
              {cellNowWords(now).replace(/^[^:]*: [^,]*, /, '')}
            </StatusLine>
            {(now.record !== null || now.open !== null) && (
              <StatusLine word="Runs">
                {openRun(now.record, 'its record, run')} {openRun(now.open, 'its open run, run')}
              </StatusLine>
            )}
          </>
        )
      }
      foot={foot}
      {...fold}
    >
      <PanelPart label="Why it would run" count={detail?.needs.length ?? 0} last>
        {!detail || detail.needs.length === 0 ? (
          <p className="muted">It would not: its record knows the current meaning of every block it reads</p>
        ) : (
          <ul>
            {detail.needs.map((c, i) => (
              <ListRow key={i}>
                {c.kind === 'changed' && <span className="mono">{c.block}</span>}
                <span className={c.kind === 'changed' && c.behind ? 'way-back' : 'muted'}>{causeRest(c)}</span>
              </ListRow>
            ))}
          </ul>
        )}
      </PanelPart>
      <PanelPart label="Reads" count={detail?.inputs.length ?? 0}>
        <ul>
          {(detail?.inputs ?? []).map((input) => (
            <ListRow key={input.block}>
              <span className="mono">{input.block}</span>
              <span className="muted">
                now <span className="mono">@{input.current ?? 'none'}</span>, its record knows{' '}
                <span className="mono">@{input.known ?? 'none'}</span>
              </span>
              {input.changed && <span className="changed-word">changed in meaning</span>}
            </ListRow>
          ))}
        </ul>
      </PanelPart>
    </Panel>
  );
}

/// One block's versions: who wrote each, when, its value, whether it changed the meaning, and its content.
function BlockVersions({ block, reader }: { block: Block; reader: Reader }) {
  return (
    <PanelPart label={<span className="mono">{block.name}</span>} count={block.versions.length}>
      <ul>
        {block.versions.map((v) => (
          <ListRow key={v.at}>
            <span className="mono muted">@{v.at}</span>
            <span>{writerWords(v.by)}</span>
            <span className="muted small" title={fullTime(v.time)}>
              {clock(v.time)}
            </span>
            {v.value !== null && <span className="mono">= {v.value}</span>}
            <span className={v.meaning === v.at ? 'changed-word' : 'muted'}>
              {v.meaning === v.at ? 'changed the meaning' : `same meaning as @${v.meaning}`}
            </span>
            <span className="muted small">{chars(v.chars)}</span>
            <Opener label="content" load={() => reader.version(block.name, v.at)} />
          </ListRow>
        ))}
      </ul>
    </PanelPart>
  );
}

/// With nothing chosen: every block that has versions.
export function BlocksPanel({ page, reader, fold, foot }: PanelProps) {
  return (
    <Panel
      label="Blocks"
      title="Blocks"
      belongs="of the work"
      lines={
        <StatusLine word={`${page.blocks.length} block${page.blocks.length === 1 ? '' : 's'}`}>
          with versions; choose a step or a cell's head to see it here
        </StatusLine>
      }
      foot={foot}
      {...fold}
    >
      {page.blocks.length === 0 && <p className="muted">No block has a version yet</p>}
      {page.blocks.map((b) => (
        <BlockVersions key={b.name} block={b} reader={reader} />
      ))}
    </Panel>
  );
}

/// The panel for what is chosen.
export function ChosenPanel(props: PanelProps & { chosen: Chosen }) {
  const { chosen, page } = props;
  if (chosen && 'cell' in chosen) return <WorkCellPanel {...props} cell={chosen.cell} />;
  const step = chosen && 'step' in chosen ? page.steps[chosen.step] : undefined;
  if (!step) return <BlocksPanel {...props} />;
  switch (step.kind) {
    case 'run':
      return <RunPanel {...props} run={findRun(page.steps, step.run) ?? step} />;
    case 'wrote':
      return <WritePanel {...props} step={step} />;
    default:
      return <ActPanel {...props} step={step} />;
  }
}
