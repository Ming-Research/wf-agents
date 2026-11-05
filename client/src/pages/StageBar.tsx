// How far a work has come, as one row of narrow cells, one for each stage of its flow, each a single letter: the stages
// it went through filled in the colour of done, the one it is at now standing out, in the colour of waiting while it
// waits, the stages ahead plain. The stages and their letters come from the flow version the work runs on, as the chart
// names them (`stagesOf`), so that a flow with other stages is read right; a work that goes back to an earlier stage
// has the later cells plain again, since the bar is read from the stage it is at. Each cell says its stage on hovering.

import type { Compiled, WorkState, WorkSummary } from '../api';
import type { Course } from '../components';
import { stagesOf } from '../graph/flowModel';
import { useResource } from '../live';
import { t } from '../text';

/// How a stage's cell looks: gone through, where the work is, where it waits, or ahead.
export type StageLook = 'done' | 'now' | 'wait' | 'ahead';

/// The look of each stage, in order, of a work at stage `at`: those before it done, it now or, while the work waits,
/// waiting, those after it ahead; a work done has every stage done, and one that ended before its end has no stage now.
/// A work at no stage the flow names has none done.
export function stageLooks(stages: readonly string[], at: string | null, state: WorkState, waiting: boolean): StageLook[] {
  if (state === 'done') return stages.map(() => 'done');
  const here = at !== null ? stages.indexOf(at) : -1;
  return stages.map((_, i): StageLook => {
    if (i < here) return 'done';
    if (i > here || here < 0) return 'ahead';
    return state === 'ended' ? 'ahead' : waiting ? 'wait' : 'now';
  });
}

/// The cells of the bar for stages already known.
export function StageCells({ stages, at, state, waiting }: {
  stages: ReadonlyArray<{ stage: string; letter: string }>;
  at: string | null;
  state: WorkState;
  waiting: boolean;
}) {
  const looks = stageLooks(
    stages.map((s) => s.stage),
    at,
    state,
    waiting,
  );
  return (
    <span className="stage-bar">
      {stages.map((s, i) => {
        const name = s.stage.replaceAll('_', ' ');
        const title = looks[i] === 'now' ? t('stage_now', { stage: name }) : looks[i] === 'wait' ? t('stage_waits', { stage: name }) : name;
        return (
          <span
            key={s.stage}
            className={`stage-cell stage-${looks[i]}`}
            title={title}
            aria-label={title}
            aria-current={looks[i] === 'now' || looks[i] === 'wait' ? 'step' : undefined}
          >
            {s.letter}
          </span>
        );
      })}
    </span>
  );
}

/// A work's bar, from the compiled flow of the version it runs on, which the page reads once for every work of the
/// version; none until it is read. `waiting` is whether it waits for something now.
export function StageBar({ w, c }: { w: WorkSummary; c: Course }) {
  const compiled = useResource<Compiled>(null, `/api/v1/flows/${w.flow_version}`);
  const g = compiled.data?.graphs.find((x) => x.name === w.graph);
  if (g === undefined) return null;
  return <StageCells stages={stagesOf(g)} at={w.stage} state={w.state} waiting={c.waits !== null} />;
}
