// A work, for diagnosis, under its row in the list of works (which holds its facts): the state machine of the flow
// version it is pinned to, filling the rest of the window, with what the work did on it: the cell it is at ringed,
// each cell it visited with its rounds and the rounds sent back or failed, the arrows it took standing out. Its rounds
// are listed on the left, under its next round while the engine holds; a round opens in the panel with its cell
// brought into view, a cell with its rounds.

import { useEffect, useRef, useState } from 'react';

import type { Compiled, CompiledGraph, FlowView, NextRound, RoundSummary, WorkDetail } from '../api';
import { Loaded, OutcomeBadge, Section } from '../components';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration } from '../format';
import { overlays, taken, chart } from '../graph/flowModel';
import { postJson } from '../http';
import { reread, useActivity, useResource, useStatus } from '../live';
import { href, navigate } from '../route';
import { t } from '../text';
import { AskView, DesignCardView, words } from './Decide';
import { RoundView } from './RoundView';

/// Who did a round's work: an agent, by its line; the engine, also for a gate it answered itself; the tools; the
/// owner.
function who(r: RoundSummary, g: CompiledGraph): string {
  if (r.line !== null && r.not_asked === null) return r.line;
  const kind = r.cell !== null ? g.cells.find((c) => c.id === r.cell)?.kind : undefined;
  return kind === 'tool' ? t('round_by_tools') : kind === 'owner' ? t('round_by_owner') : t('round_by_engine');
}

/// The rounds, newest first: the cell and who did its work, the row it took (a badge only for what is not the usual
/// course: sent back, failed, cut off, running), what an agent's round cost and how long it took.
function Timeline(props: {
  rounds: RoundSummary[];
  g: CompiledGraph;
  selected: number | null;
  onRound: (r: RoundSummary) => void;
}) {
  if (props.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  const newest = [...props.rounds].reverse();
  return (
    <table className="list timeline">
      <tbody>
        {newest.map((r) => {
          const what = r.not_asked !== null ? t('round_not_asked', { need: words(r.not_asked) }) : (r.transition ?? r.why ?? '');
          return (
            <tr
              key={r.id}
              className={`clickable${r.id === props.selected ? ' selected' : ''}`}
              onClick={() => props.onRound(r)}
            >
              <td className="muted">#{r.id}</td>
              <td className="nowrap">
                <span className="cell-id">{r.cell ?? '–'}</span> <span className="muted">{who(r, props.g)}</span>
              </td>
              <td className="clip" title={what}>
                {r.outcome !== 'took' ? <OutcomeBadge outcome={r.outcome} /> : null}{' '}
                <span className={r.not_asked !== null ? 'muted' : undefined}>{what}</span>
              </td>
              <td className="num muted">{r.cost_usd !== null && r.line !== null ? cost(r.cost_usd) : ''}</td>
              <td className="num muted">{r.ended_at !== null ? duration(r.ended_at - r.started_at) : ''}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/// The work's next round while the engine holds: what it will be, the button that runs it, and while it runs, how long
/// it has run and what the agent is doing; when it ends, its round opens.
function Next({ d, onRound }: { d: WorkDetail; onRound: (r: number) => void }) {
  const status = useStatus();
  const w = d.work;
  const activity = useActivity(w.id);
  const asked = useRef(false);
  // When the owner pressed the button: until the round asked for has ended, the button says it is starting.
  const [pressed, setPressed] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const stepping = status?.stepping === w.id;
  const can = status !== null && status.mode === 'hold' && status.halt === null && status.stepping === null && w.state === 'ready';
  const next = useResource<NextRound>(`work:${w.id}`, can ? `/api/v1/works/${w.id}/next` : null);
  useEffect(() => {
    if (!stepping) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [stepping]);
  // The round asked for has ended: open it.
  const last = status?.last_step;
  useEffect(() => {
    if (!asked.current || stepping || !last || last.work !== w.id) return;
    asked.current = false;
    const r = last.rounds[last.rounds.length - 1];
    if (r !== undefined) onRound(r);
  }, [stepping, last, w.id, onRound]);
  if (stepping) {
    const since = w.running?.started_at ?? null;
    return (
      <section className="next">
        <div className="next-head">
          <strong>{t('next_running')}</strong>
          {since !== null ? <span className="muted">{duration(now - since)}</span> : null}
        </div>
        <ol className="next-activity">
          {activity.map((a, i) => (
            <li key={i}>{a.what}</li>
          ))}
        </ol>
      </section>
    );
  }
  if (!can) return null;
  const n = next.data;
  const starting = pressed !== null && !(last !== null && last !== undefined && last.work === w.id && last.at >= pressed);
  const kind =
    n === undefined
      ? ''
      : n.not_asked !== null
        ? t('next_kind_not_asked')
        : n.kind === 'agent'
          ? t('next_kind_agent', { line: n.line ?? '' })
        : n.kind === 'engine'
          ? t('next_kind_engine', { step: n.step ?? '' })
          : n.kind === 'tool'
            ? t('next_kind_tool')
            : t('next_kind_owner');
  const run = () => {
    setError(null);
    setPressed(Date.now());
    asked.current = true;
    postJson(`/api/v1/works/${w.id}/step`, {}).catch((e: unknown) => {
      asked.current = false;
      setPressed(null);
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  const note =
    n === undefined
      ? ''
      : n.not_asked !== null
        ? t('next_no_agent')
        : n.kind === 'agent'
          ? t('next_real')
          : n.kind === 'tool'
            ? t('next_tools_note')
            : n.kind === 'owner'
              ? t('next_owner_note')
              : t('next_engine_note');
  return (
    <section className="next">
      <div className="next-head">
        <strong>{t('next_title')}</strong>
        {n !== undefined ? (
          <span>
            <span className="cell-id">{n.cell}</span> {n.title} · <span className="muted">{kind}</span>
          </span>
        ) : null}
      </div>
      {next.error !== null ? <p className="bad">{next.error}</p> : null}
      {n !== undefined && n.not_asked !== null ? <p className="prose">{n.not_asked}</p> : null}
      {n !== undefined && n.tools.length > 0 ? (
        <ul className="next-tools">
          {n.tools.map((x, i) => (
            <li key={i}>{x}</li>
          ))}
        </ul>
      ) : null}
      {n?.message ? (
        <details>
          <summary>{t('next_message')}</summary>
          <pre className="j-text next-message">{n.message}</pre>
        </details>
      ) : null}
      <div className="next-run">
        <button type="button" className="next-button" onClick={run} disabled={n === undefined || starting}>
          {starting ? t('next_starting') : t('next_run')}
        </button>
        <span className="muted">{note}</span>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </section>
  );
}

/// Why the work is paused, and the button that lets it go on, once the owner confirms the cell that runs again.
function Paused({ d }: { d: WorkDetail }) {
  const w = d.work;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const go = () => {
    const cell = `${w.at}${w.at_title ? ` (${w.at_title})` : ''}`;
    if (!window.confirm(t('resume_confirm', { id: w.id, cell }))) return;
    setBusy(true);
    setError(null);
    postJson(`/api/v1/works/${w.id}/resume`, {}).then(
      () => reread('works', `work:${w.id}`),
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return (
    <section className="next paused">
      <strong>{t('paused_title')}</strong>
      <p className="prose">{w.paused}</p>
      <div className="next-run">
        <button type="button" className="decide-button" onClick={go} disabled={busy}>
          {t('resume')}
        </button>
        <span className="muted">{t('resume_note', { cell: w.at })}</span>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </section>
  );
}

/// The work's rounds and records, in a drawer over the left of the chart.
function Drawer({ d, g, round, onRound }: {
  d: WorkDetail;
  g: CompiledGraph;
  round: number | null;
  onRound: (r: number) => void;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div className={`drawer${open ? '' : ' drawer-closed'}`}>
      <button type="button" className="drawer-toggle" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        {open ? '‹' : '›'} {t('rounds')} ({d.rounds.length})
      </button>
      {open ? (
        <div className="drawer-body">
          {d.work.paused !== null ? <Paused d={d} /> : null}
          <Next d={d} onRound={onRound} />
          <Timeline rounds={d.rounds} g={g} selected={round} onRound={(r) => onRound(r.id)} />
          {d.messages.length > 0 ? (
            <Section title={t('work_messages')} open={false}>
              {d.messages.map((m) => (
                <div key={m.id} className="boxed">
                  <div className="muted">
                    {m.cell !== null ? t('message_cell', { cell: m.cell }) : null}
                    {m.round !== null ? (
                      <>
                        {' · '}
                        <button className="link" onClick={() => (m.round !== null ? onRound(m.round) : undefined)}>
                          {t('round_title', { id: m.round })}
                        </button>
                      </>
                    ) : null}
                  </div>
                  <p className="prose">{m.text}</p>
                </div>
              ))}
            </Section>
          ) : null}
          {d.asks.length > 0 ? (
            <Section title={t('asks')} open={false}>
              {d.asks.map((a) => (
                <div key={a.id} className="boxed">
                  <div>
                    #{a.id} · {a.answer === null ? t('ask_waits') : t('ask_answered')} ·{' '}
                    <button className="link" onClick={() => onRound(a.round)}>
                      {t('round_title', { id: a.round })}
                    </button>
                  </div>
                  <AskView a={a} />
                </div>
              ))}
            </Section>
          ) : null}
          {d.design_cards.length > 0 ? (
            <Section title={t('design_cards')} open={false}>
              {d.design_cards.map((c) => (
                <div key={c.id} className="boxed">
                  <div>
                    #{c.id} · {c.status} ·{' '}
                    <button className="link" onClick={() => onRound(c.round)}>
                      {t('round_title', { id: c.round })}
                    </button>
                  </div>
                  <DesignCardView c={c} />
                </div>
              ))}
            </Section>
          ) : null}
          {d.interjections.length > 0 ? (
            <Section title={t('interjections')} open={false}>
              {d.interjections.map((x) => (
                <div key={x.id} className="boxed">
                  <div className="muted">
                    #{x.id} {x.speaker} → {x.line} ·{' '}
                    {x.delivered_round !== null
                      ? t('delivered', { round: x.delivered_round })
                      : t('waiting_delivery')}
                  </div>
                  <pre className="j-text">{x.text}</pre>
                </div>
              ))}
            </Section>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function Body({ id, d, compiled, view, round, cell }: {
  id: number;
  d: WorkDetail;
  compiled: Compiled;
  view: FlowView;
  round: number | null;
  cell: string | null;
}) {
  const g = compiled.graphs.find((x) => x.name === d.work.graph);
  const v = view.graphs.find((x) => x.name === d.work.graph);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const roundCell = round !== null ? (d.rounds.find((r) => r.id === round)?.cell ?? null) : null;
  const go = (r: number | null, c: string | null) => navigate(href({ page: 'work', id, round: r, cell: c }), true);
  const model = chart(g, v);
  return (
    <>
      <Drawer d={d} g={g} round={round} onRound={(r) => go(r, null)} />
      <FlowScreen
        compiled={compiled}
        g={g}
        view={v}
        legend="work"
        marks={{
          overlays: overlays(d.rounds, d.work.at),
          taken: taken(model, g, d.rounds),
          added: new Set(),
          changed: new Set(),
        }}
        cell={cell}
        onCell={(c) => go(null, c)}
        start={roundCell ?? d.work.at}
        rounds={d.rounds}
        round={round}
        onRound={(r) => go(r, null)}
        renderRound={(r) => <RoundView id={r} rounds={d.rounds} />}
      />
    </>
  );
}

export function Work({ id, round, cell }: { id: number; round: number | null; cell: string | null }) {
  const detail = useResource<WorkDetail>(`work:${id}`, `/api/v1/works/${id}`);
  const version = detail.data?.work.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  return (
    <div className="screen">
      <Loaded r={detail}>
        {(d) => (
          <>
            <Loaded r={compiled}>
              {(c) => (
                <Loaded r={view}>
                  {(v) => <Body id={id} d={d} compiled={c} view={v} round={round} cell={cell} />}
                </Loaded>
              )}
            </Loaded>
          </>
        )}
      </Loaded>
    </div>
  );
}
