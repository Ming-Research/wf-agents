// Where the item's latest work stands, as a checklist under the flow's stages: the cells done, ticked; the cell it is
// at now, with what it waits for and its latest progress note, open; the cells ahead on the main path. Each line opens
// to its progress notes, newest first, and leads to its rounds on the work's chart.

import { CheckCircleFillIcon, CircleIcon, DotFillIcon } from '@primer/octicons-react';
import { Link as PrimerLink } from '@primer/react';

import type { Compiled, CompiledGraph, ItemDetail, RoundSummary, WorkDetail, WorkSummary } from '../api';
import { liveState, Loaded, OutcomeBadge } from '../components';
import { words } from '../labelled';
import { useResource, useStatus } from '../live';
import { MarkdownInline } from '../markdown';
import { RoundLink, SubHeading } from '../parts';
import { bringIntoView } from '../pages/Decide';
import { Link } from '../route';
import { t, tn } from '../text';
import { byStage, progress, type Step } from './checklist';

/// What the work waits for at the cell it is at, in words, with the design card or ask it waits on brought into view.
function Waits({ d, w }: { d: ItemDetail; w: WorkSummary }) {
  const status = useStatus();
  const card = d.design_cards.find((c) => c.work === w.id && c.status === 'open');
  const ask = d.asks.find((a) => a.work === w.id && a.answer === null);
  const show = (id: string) => () => bringIntoView(id);
  if (card !== undefined) {
    return (
      <PrimerLink as="button" type="button" className="step-waits" onClick={show(`design-card-${card.id}`)}>
        {t('progress_waits_card', { id: card.id })}
      </PrimerLink>
    );
  }
  if (ask !== undefined) {
    return (
      <PrimerLink as="button" type="button" className="step-waits" onClick={show(`ask-${ask.id}`)}>
        {t('progress_waits_ask', { id: ask.id })}
      </PrimerLink>
    );
  }
  if (w.paused !== null) {
    const key =
      w.paused_by === 'architect'
        ? 'progress_stopped_architect'
        : w.paused_by === 'engine'
          ? 'progress_halted'
          : 'progress_paused';
    return <p className="step-waits warn">{t(key, { why: w.paused })}</p>;
  }
  const state = liveState(w, status);
  const key =
    state === 'running'
      ? 'progress_running'
      : state === 'waiting'
        ? 'progress_waiting'
        : state === 'waiting_for_slot'
          ? 'progress_waiting_for_slot'
          : state === 'asking'
            ? 'progress_asking'
            : 'progress_ready';
  return <p className="step-waits muted">{t(key)}</p>;
}

/// What a round under a line said: its progress note, or why it did not take a row.
function Note({ r, work }: { r: RoundSummary; work: number }) {
  const bad = r.outcome === 'sent_back' || r.outcome === 'failed' || r.outcome === 'interrupted';
  if (r.progress === null && !bad) return null;
  return (
    <li>
      <RoundLink work={work} round={r.n} />{' '}
      {bad ? (
        <>
          <OutcomeBadge outcome={r.outcome} /> <span className="muted">{r.why ?? ''}</span>
        </>
      ) : (
        <MarkdownInline text={r.progress ?? ''} />
      )}
    </li>
  );
}

/// A cell done, the cell the work is at, and a cell ahead.
const MARK = { done: CheckCircleFillIcon, now: DotFillIcon, ahead: CircleIcon } as const;

function Line({ s, d, w }: { s: Step; d: ItemDetail; w: WorkSummary }) {
  const Mark = MARK[s.state];
  const head = (
    <>
      <span className="step-mark" title={t(`progress_${s.state}`)}>
        <Mark />
      </span>
      <span className="cell-id">{s.cell}</span> <span className="step-title">{s.title}</span>
      {s.passes > 1 ? (
        <span className="muted" title={tn('progress_passes_title', s.passes)}>
          {' '}
          ×{s.passes}
        </span>
      ) : null}
      {s.row !== null && s.state !== 'ahead' ? <code className="step-row">{s.row}</code> : null}
    </>
  );
  if (s.state === 'ahead') return <div className="step step-ahead">{head}</div>;
  const notes = [...s.rounds].reverse();
  return (
    <details className={`step step-${s.state}`} open={s.state === 'now'}>
      <summary>{head}</summary>
      <div className="step-body">
        {s.state === 'now' ? <Waits d={d} w={w} /> : null}
        <ul className="step-notes">
          {notes.map((r) => (
            <Note key={r.n} r={r} work={w.id} />
          ))}
        </ul>
        <Link to={`/works/${w.id}?cell=${encodeURIComponent(s.cell)}`} className="shown-from">
          {t('progress_on_chart')}
        </Link>
      </div>
    </details>
  );
}

function Checklist({ d, detail, g }: { d: ItemDetail; detail: WorkDetail; g: CompiledGraph }) {
  const w = detail.work;
  const live = w.state !== 'done' && w.state !== 'ended';
  const steps = progress(g, detail.rounds, { at: w.at, resumeAt: detail.resume_at, live });
  if (steps.length === 0) return <p className="muted">{t('progress_none')}</p>;
  return (
    <div className="checklist">
      {w.state === 'ended' ? <p className="warn">{t('progress_stopped', { why: w.ended ?? '' })}</p> : null}
      {byStage(g, steps).map((group) => (
        <div key={group.stage ?? ''} className="checklist-stage">
          {group.stage !== null ? <SubHeading className="checklist-stage-name">{words(group.stage)}</SubHeading> : null}
          {group.steps.map((s) => (
            <Line key={s.cell} s={s} d={d} w={w} />
          ))}
        </div>
      ))}
    </div>
  );
}

/// The checklist of the item's latest work.
export function Progress({ d }: { d: ItemDetail }) {
  const w = d.works.length > 0 ? d.works[d.works.length - 1] : null;
  const detail = useResource<WorkDetail>(w !== null ? `work:${w.id}` : null, w !== null ? `/api/v1/works/${w.id}` : null);
  // A stored flow version never changes: it is read once.
  const compiled = useResource<Compiled>(null, w !== null ? `/api/v1/flows/${w.flow_version}` : null);
  if (w === null) return <p className="muted">{t('progress_none')}</p>;
  return (
    <Loaded r={detail}>
      {(x) => (
        <Loaded r={compiled}>
          {(c) => {
            const g = c.graphs.find((y) => y.name === x.work.graph);
            return g === undefined ? <p className="muted">{t('none')}</p> : <Checklist d={d} detail={x} g={g} />;
          }}
        </Loaded>
      )}
    </Loaded>
  );
}
