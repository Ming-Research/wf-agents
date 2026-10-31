// A flow's state machine: a stored version, or the flow as `flow/flow.json` is at a git ref of a project's checkout,
// filling the window under the bar, its graphs (the engineer's and the architect's) side by side on one chart that
// moves and zooms as one. Over its top left corner, which flow is shown and, to review a change, which one it is
// compared with; a comparison marks the cells added and changed in every graph and lists what changed.

import { ActionList, ActionMenu, Button, Flash, Heading, Label, Radio, Select, TextInput } from '@primer/react';
import { useMemo, useState } from 'react';

import type { Compiled, FlowAt, FlowDiff, FlowSource, FlowVersion, FlowView } from '../api';
import { Loaded } from '../components';
import { FlowScreen } from '../flow/FlowScreen';
import type { Graph } from '../graph/FlowChart';
import { type Resource, useResource, useStatus } from '../live';
import { href, navigate, type Route } from '../route';
import { t, tn } from '../text';
import { SEP } from '../voices';

type FlowRoute = Extract<Route, { page: 'flow' }>;

const go = (r: FlowRoute, replace = false) => navigate(href(r), replace);

/// A version by its number, and whether new works start on it, as the comparison's choices say it.
function versionLabel(v: FlowVersion, current: number | null): string {
  return v.id === current ? t('flow_version_in_use', { id: v.id }) : t('flow_version_short', { id: v.id });
}

/// What to compare the shown flow with: another stored version, or the flow at a ref of a project's checkout.
function CompareBox({ r, versions, shown, current, onClose }: {
  r: FlowRoute;
  versions: FlowVersion[];
  shown: number | null;
  current: number | null;
  onClose: () => void;
}) {
  const sources = useResource<FlowSource[]>(null, '/api/v1/flows/sources');
  const [from, setFrom] = useState<number>(r.from ?? versions.find((v) => v.id !== shown)?.id ?? versions[0]?.id ?? 0);
  const [against, setAgainst] = useState<'version' | 'ref'>(r.project !== null ? 'ref' : 'version');
  const [to, setTo] = useState<number>(shown ?? versions[0]?.id ?? 0);
  const [project, setProject] = useState<string>(r.project ?? '');
  const [ref, setRef] = useState<string>(r.ref ?? '');
  const list = sources.data ?? [];
  const chosen = list.find((s) => s.project === project) ?? list[0];
  const show = () => {
    if (against === 'version') go({ ...r, version: to, project: null, ref: null, from, graph: null, cell: null });
    else if (chosen && ref.trim() !== '') {
      go({ ...r, version: null, project: chosen.project, ref: ref.trim(), from, graph: null, cell: null });
    }
    onClose();
  };
  const options = versions.map((v) => (
    <option key={v.id} value={v.id}>
      {versionLabel(v, current)}
    </option>
  ));
  return (
    <div className="compare-box" role="dialog" aria-labelledby="compare-title">
      <Heading as="h2" variant="small" id="compare-title">
        {t('compare_title')}
      </Heading>
      <label className="compare-row">
        <span>{t('compare_older')}</span>
        <Select size="small" value={String(from)} onChange={(e) => setFrom(Number(e.target.value))}>
          {options}
        </Select>
      </label>
      <div className="compare-row">
        <label className="compare-choice">
          <Radio name="compare-against" value="version" checked={against === 'version'} onChange={() => setAgainst('version')} />
          {t('compare_with_version')}
        </label>
        <Select
          size="small"
          value={String(to)}
          disabled={against !== 'version'}
          aria-label={t('compare_with_version')}
          onChange={(e) => setTo(Number(e.target.value))}
        >
          {options}
        </Select>
      </div>
      <div className="compare-row">
        <label className="compare-choice">
          <Radio name="compare-against" value="ref" checked={against === 'ref'} onChange={() => setAgainst('ref')} />
          {t('compare_with_ref')}
        </label>
        <Select
          size="small"
          value={chosen?.project ?? ''}
          disabled={against !== 'ref'}
          aria-label={t('compare_project')}
          onChange={(e) => setProject(e.target.value)}
        >
          {list.map((s) => (
            <option key={s.project} value={s.project} title={s.checkout}>
              {s.project}
            </option>
          ))}
        </Select>
        <TextInput
          size="small"
          list="flow-refs"
          value={ref}
          disabled={against !== 'ref'}
          placeholder={t('compare_ref_hint')}
          aria-label={t('compare_ref_hint')}
          onChange={(e) => setRef(e.target.value)}
        />
        <datalist id="flow-refs">
          {(chosen?.refs ?? []).map((x) => (
            <option key={x} value={x} />
          ))}
        </datalist>
      </div>
      {sources.error !== null ? <Flash variant="danger">{t('error', { error: sources.error })}</Flash> : null}
      <div className="compare-actions">
        <Button size="small" onClick={onClose}>
          {t('close')}
        </Button>
        <Button size="small" variant="primary" onClick={show}>
          {t('compare_show')}
        </Button>
      </div>
    </div>
  );
}

/// Over the chart's top left corner, on one surface: which flow is shown, as a menu of the stored versions (the one new
/// works start on marked in use, each with its digest and how many works are pinned to it), or the git ref it was read
/// at; what it is compared with; and Compare, which opens the choice of what to compare it with.
function Bar({ r, versions, current, at }: {
  r: FlowRoute;
  versions: FlowVersion[];
  current: number | null;
  at: Resource<FlowAt> | null;
}) {
  const [comparing, setComparing] = useState(false);
  const shown = r.version ?? current;
  return (
    <div className="flow-tools">
      {r.project !== null ? (
        <span className="flow-at" title={at?.data?.commit ?? ''}>
          {t('flow_at_ref', { project: r.project, ref: r.ref ?? '' })}
          {at?.data?.version != null ? ` = ${t('flow_version_short', { id: at.data.version })}` : ''}
        </span>
      ) : (
        <ActionMenu>
          <ActionMenu.Button size="small" aria-label={t('flow_version_pick')}>
            {t('flow_menu', { id: shown ?? '' })}
          </ActionMenu.Button>
          <ActionMenu.Overlay width="medium">
            <ActionList selectionVariant="single" aria-label={t('flow_version_pick')}>
              {versions.map((v) => (
                <ActionList.Item
                  key={v.id}
                  selected={v.id === shown}
                  onSelect={() => go({ ...r, version: v.id, graph: null, cell: null })}
                >
                  {t('flow_version_short', { id: v.id })}
                  <ActionList.Description>
                    <span className="cell-id">{v.digest.slice(0, 8)}</span>
                    {SEP}
                    {tn('flow_works', v.works)}
                  </ActionList.Description>
                  {v.id === current ? (
                    <ActionList.TrailingVisual>
                      <Label variant="success" title={t('flow_current')}>
                        {t('flow_in_use')}
                      </Label>
                    </ActionList.TrailingVisual>
                  ) : null}
                </ActionList.Item>
              ))}
            </ActionList>
          </ActionMenu.Overlay>
        </ActionMenu>
      )}
      {r.from !== null ? (
        <span className="flow-compared">
          {t('flow_compared', { id: r.from })}
          <Button
            size="small"
            variant="invisible"
            onClick={() => go({ ...r, from: null, project: null, ref: null, version: r.project !== null ? null : r.version })}
          >
            {t('compare_stop')}
          </Button>
        </span>
      ) : null}
      <Button size="small" onClick={() => setComparing((c) => !c)} aria-expanded={comparing}>
        {t('compare_open')}
      </Button>
      {comparing ? (
        <CompareBox r={r} versions={versions} shown={shown} current={current} onClose={() => setComparing(false)} />
      ) : null}
    </div>
  );
}

/// What a comparison marks on each graph: the cells added and the cells changed.
export function diffMarks(graphs: Graph[], diff: FlowDiff | null) {
  return graphs.map(({ g }) => {
    const changes = diff?.graphs.find((x) => x.name === g.name);
    return {
      overlays: null,
      taken: null,
      added: new Set(changes?.cells.filter((c) => c.status === 'added').map((c) => c.id) ?? []),
      changed: new Set(changes?.cells.filter((c) => c.status === 'changed').map((c) => c.id) ?? []),
    };
  });
}

/// Every graph of the flow, side by side on the one chart.
function Shown({ r, compiled, view, diff }: { r: FlowRoute; compiled: Compiled; view: FlowView; diff: FlowDiff | null }) {
  const graphs = useMemo(
    () =>
      compiled.graphs.flatMap((g) => {
        const v = view.graphs.find((x) => x.name === g.name);
        return v ? [{ g, view: v }] : [];
      }),
    [compiled, view],
  );
  const marks = useMemo(() => diffMarks(graphs, diff), [graphs, diff]);
  if (graphs.length === 0) return <p className="muted page">{t('none')}</p>;
  return (
    <FlowScreen
      key={`${r.version ?? ''}:${r.project ?? ''}:${r.ref ?? ''}`}
      compiled={compiled}
      graphs={graphs}
      legend={diff ? 'compare' : 'flow'}
      marks={marks}
      cell={r.cell}
      cellGraph={r.graph}
      onCell={(cell, graph) => go({ ...r, graph: cell !== null ? (graph ?? null) : null, cell }, true)}
      start={r.cell}
      startGraph={r.graph}
      diff={diff}
    />
  );
}

function Stored({ r, version }: { r: FlowRoute; version: number }) {
  const compiled = useResource<Compiled>(null, `/api/v1/flows/${version}`);
  const view = useResource<FlowView>(null, `/api/v1/flows/${version}/view`);
  const diff = useResource<FlowDiff>(null, r.from !== null ? `/api/v1/flows/diff?from=${r.from}&to=${version}` : null);
  return (
    <Loaded r={compiled}>
      {(c) => (
        <Loaded r={view}>
          {(v) => <Shown r={r} compiled={c} view={v} diff={r.from !== null ? (diff.data ?? null) : null} />}
        </Loaded>
      )}
    </Loaded>
  );
}

function AtRef({ r, at }: { r: FlowRoute; at: Resource<FlowAt> }) {
  const q = `project=${encodeURIComponent(r.project ?? '')}&ref=${encodeURIComponent(r.ref ?? '')}`;
  const diff = useResource<FlowDiff>(null, r.from !== null ? `/api/v1/flows/diff?from=${r.from}&${q}` : null);
  return (
    <Loaded r={at}>
      {(a) => <Shown r={r} compiled={a.compiled} view={a.view} diff={r.from !== null ? (diff.data ?? null) : null} />}
    </Loaded>
  );
}

export function Flow({ route }: { route: FlowRoute }) {
  const status = useStatus();
  const versions = useResource<FlowVersion[]>('flows', '/api/v1/flows');
  const atRef = route.project !== null && route.ref !== null;
  const q = atRef ? `project=${encodeURIComponent(route.project ?? '')}&ref=${encodeURIComponent(route.ref ?? '')}` : null;
  const at = useResource<FlowAt>(null, q !== null ? `/api/v1/flows/at?${q}` : null);
  return (
    <div className="screen">
      <Loaded r={versions}>
        {(all) => {
          const current = status?.flow_version ?? all[0]?.id ?? null;
          const chosen = route.version ?? current;
          return (
            <>
              <Bar r={route} versions={all} current={status?.flow_version ?? null} at={atRef ? at : null} />
              {atRef ? <AtRef r={route} at={at} /> : chosen !== null ? <Stored r={route} version={chosen} /> : null}
            </>
          );
        }}
      </Loaded>
    </div>
  );
}
