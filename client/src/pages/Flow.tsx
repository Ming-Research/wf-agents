// A flow's state machine: a stored version, or the flow as `flow/flow.json` is at a git ref of a project's checkout,
// filling the window under the bar. Over its top left corner, which flow is shown and, to review a change, which one
// it is compared with; a comparison marks the cells added and changed and lists what changed.

import { useState } from 'react';

import type { Compiled, FlowAt, FlowDiff, FlowSource, FlowVersion, FlowView } from '../api';
import { Loaded } from '../components';
import { FlowScreen } from '../flow/FlowScreen';
import { type Resource, useResource, useStatus } from '../live';
import { href, navigate, type Route } from '../route';
import { t } from '../text';

type FlowRoute = Extract<Route, { page: 'flow' }>;

const go = (r: FlowRoute, replace = false) => navigate(href(r), replace);

function versionLabel(v: FlowVersion): string {
  return t('flow_version_short', { id: v.id, digest: v.digest.slice(0, 8) });
}

/// What to compare the shown flow with: another stored version, or the flow at a ref of a project's checkout.
function CompareBox({ r, versions, shown, onClose }: {
  r: FlowRoute;
  versions: FlowVersion[];
  shown: number | null;
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
    if (against === 'version') go({ ...r, version: to, project: null, ref: null, from, cell: null });
    else if (chosen && ref.trim() !== '') {
      go({ ...r, version: null, project: chosen.project, ref: ref.trim(), from, cell: null });
    }
    onClose();
  };
  return (
    <div className="bar-pop" role="dialog" aria-label={t('compare_title')}>
      <div className="bar-pop-h">{t('compare_title')}</div>
      <label>
        {t('compare_older')}{' '}
        <select value={from} onChange={(e) => setFrom(Number(e.target.value))}>
          {versions.map((v) => (
            <option key={v.id} value={v.id}>
              {versionLabel(v)}
            </option>
          ))}
        </select>
      </label>
      <div className="bar-pop-row">
        <label>
          <input type="radio" checked={against === 'version'} onChange={() => setAgainst('version')} />{' '}
          {t('compare_with_version')}
        </label>
        <select value={to} disabled={against !== 'version'} onChange={(e) => setTo(Number(e.target.value))}>
          {versions.map((v) => (
            <option key={v.id} value={v.id}>
              {versionLabel(v)}
            </option>
          ))}
        </select>
      </div>
      <div className="bar-pop-row">
        <label>
          <input type="radio" checked={against === 'ref'} onChange={() => setAgainst('ref')} />{' '}
          {t('compare_with_ref')}
        </label>
        <select
          value={chosen?.project ?? ''}
          disabled={against !== 'ref'}
          onChange={(e) => setProject(e.target.value)}
        >
          {list.map((s) => (
            <option key={s.project} value={s.project} title={s.checkout}>
              {s.project}
            </option>
          ))}
        </select>
        <input
          list="flow-refs"
          value={ref}
          disabled={against !== 'ref'}
          placeholder={t('compare_ref_hint')}
          onChange={(e) => setRef(e.target.value)}
        />
        <datalist id="flow-refs">
          {(chosen?.refs ?? []).map((x) => (
            <option key={x} value={x} />
          ))}
        </datalist>
      </div>
      {sources.error !== null ? <div className="error">{t('error', { error: sources.error })}</div> : null}
      <div className="bar-pop-row">
        <button type="button" className="tog-btn" onClick={show}>
          {t('compare_show')}
        </button>
        <button type="button" className="tog-btn" onClick={onClose}>
          {t('close')}
        </button>
      </div>
    </div>
  );
}

function Bar({ r, versions, current, at }: {
  r: FlowRoute;
  versions: FlowVersion[];
  current: number | null;
  at: Resource<FlowAt> | null;
}) {
  const [comparing, setComparing] = useState(false);
  const shown = r.version ?? current;
  const picked = versions.find((v) => v.id === shown);
  const title = picked
    ? [
        t('flow_digest', { digest: picked.digest }),
        t('flow_works', { n: picked.works }),
        picked.id === current ? t('flow_current') : null,
      ]
        .filter((x) => x !== null)
        .join('\n')
    : '';
  return (
    <div className="flow-tools">
      <div className="bar-flow">
        {r.project !== null ? (
          <span className="bar-at" title={at?.data?.commit ?? ''}>
            {t('flow_at_ref', { project: r.project, ref: r.ref ?? '' })}
            {at?.data?.version != null ? ` = ${t('flow_version_short', { id: at.data.version, digest: at.data.digest.slice(0, 8) })}` : ''}
          </span>
        ) : (
          <select
            className="bar-version"
            value={shown ?? ''}
            title={title}
            aria-label={t('flow_version_pick')}
            onChange={(e) => go({ ...r, version: Number(e.target.value), cell: null })}
          >
            {versions.map((v) => (
              <option
                key={v.id}
                value={v.id}
                title={`${t('flow_works', { n: v.works })}${v.id === current ? ` · ${t('flow_current')}` : ''}`}
              >
                {versionLabel(v)}
                {v.id === current ? ' ●' : ''}
              </option>
            ))}
          </select>
        )}
        {r.from !== null ? (
          <span className="bar-compare">
            {t('flow_compared', { id: r.from })}
            <button
              type="button"
              className="link"
              onClick={() => go({ ...r, from: null, project: null, ref: null, version: r.project !== null ? null : r.version })}
            >
              {t('compare_stop')}
            </button>
          </span>
        ) : null}
        <button type="button" className="bar-btn" onClick={() => setComparing((c) => !c)}>
          {t('compare_open')}
        </button>
        {comparing ? (
          <CompareBox r={r} versions={versions} shown={shown} onClose={() => setComparing(false)} />
        ) : null}
      </div>
    </div>
  );
}

function Shown({ r, compiled, view, diff }: { r: FlowRoute; compiled: Compiled; view: FlowView; diff: FlowDiff | null }) {
  const [graph, setGraph] = useState<string | null>(null);
  const g = compiled.graphs.find((x) => x.name === graph) ?? compiled.graphs[0];
  const v = view.graphs.find((x) => x.name === g?.name);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const changes = diff?.graphs.find((x) => x.name === g.name);
  const added = new Set(changes?.cells.filter((c) => c.status === 'added').map((c) => c.id) ?? []);
  const changed = new Set(changes?.cells.filter((c) => c.status === 'changed').map((c) => c.id) ?? []);
  return (
    <>
      {compiled.graphs.length > 1 ? (
        <div className="flow-tools flow-tools-graph">
          <select value={g.name} onChange={(e) => setGraph(e.target.value)} aria-label={t('flow_graph_pick')}>
            {compiled.graphs.map((x) => (
              <option key={x.name} value={x.name}>
                {x.name}
              </option>
            ))}
          </select>
        </div>
      ) : null}
      <FlowScreen
        key={`${g.name}:${r.version ?? ''}:${r.project ?? ''}:${r.ref ?? ''}`}
        compiled={compiled}
        g={g}
        view={v}
        legend={diff ? 'compare' : 'flow'}
        marks={{ overlays: null, taken: null, added, changed }}
        cell={r.cell}
        onCell={(cell) => go({ ...r, cell }, true)}
        start={r.cell}
        diff={diff}
      />
    </>
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
