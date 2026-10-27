// A work, under its row in the list of works (which holds its facts): the state machine of the flow version it is
// pinned to, filling the rest of the window, with what the work did on it: the cell it is at ringed, each cell it
// visited with its rounds and the rounds sent back or failed, the arrows it took standing out. A window over the left
// of the chart, whose width drags, has three tabs: its rounds, under its next round while the engine holds; its design
// cards, the one waiting for the owner's ruling first with its controls; and its messages, the asks waiting for the
// owner's answer first with their forms, then what its agents and the engine told the owner. A round opens in the
// panel with its cell brought into view, a cell with its rounds.

import { useEffect, useRef, useState } from 'react';

import type {
  Compiled,
  CompiledGraph,
  DesignCard,
  FlowView,
  NextRound,
  OwnerAsk,
  OwnerMessage,
  RoundSummary,
  WorkDetail,
} from '../api';
import { liveState, Loaded, OutcomeBadge } from '../components';
import { FlowScreen } from '../flow/FlowScreen';
import { savedWidth, WidthGrip } from '../flow/Panel';
import { cost, duration, time } from '../format';
import { overlays, taken, chart } from '../graph/flowModel';
import { postJson } from '../http';
import { reread, useActivity, useResource, useStatus } from '../live';
import { href, navigate, type RoundsOnly, type WorkRoute, type WorkTab } from '../route';
import { t } from '../text';
import { AskView, bringIntoView, DesignCardView, words } from './Decide';
import { markRead } from './Messages';
import { RoundView } from './RoundView';

/// Who did a round's work: an agent, by its line; the engine, also for a gate it answered itself; the tools; the
/// owner.
function who(r: RoundSummary, g: CompiledGraph): string {
  if (r.line !== null && r.not_asked === null) return r.line;
  const kind = r.cell !== null ? g.cells.find((c) => c.id === r.cell)?.kind : undefined;
  return kind === 'tool' ? t('round_by_tools') : kind === 'owner' ? t('round_by_owner') : t('round_by_engine');
}

/// The rounds the list shows: all of them, or only those whose return was sent back, or only those whose run failed or
/// was cut off, as the list of works counts them.
export function roundsShown(rounds: RoundSummary[], only: RoundsOnly | null): RoundSummary[] {
  if (only === 'sent_back') return rounds.filter((r) => r.outcome === 'sent_back');
  if (only === 'failed') return rounds.filter((r) => r.outcome === 'failed' || r.outcome === 'interrupted');
  return rounds;
}

/// The rounds, newest first: the cell and who did its work, the row it took (a badge only for what is not the usual
/// course: sent back, failed, cut off, running), what an agent's round cost and how long it took.
function Timeline(props: {
  rounds: RoundSummary[];
  g: CompiledGraph;
  /// Whether a round still open is one left over, not one running: the engine holds and runs nothing of this work.
  stale: boolean;
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
                {r.outcome === 'running' && props.stale ? (
                  <span className="badge" title={t('round_left_open_hint')}>
                    {t('round_left_open')}
                  </span>
                ) : r.outcome !== 'took' ? (
                  <OutcomeBadge outcome={r.outcome} />
                ) : null}{' '}
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
function Next({ d, onRound, onCard, onAsk }: {
  d: WorkDetail;
  onRound: (r: number) => void;
  /// Open the design card, or the ask, the work waits for, in its tab of the window.
  onCard: (id: number) => void;
  onAsk: (id: number) => void;
}) {
  const status = useStatus();
  const w = d.work;
  const activity = useActivity(w.id);
  const asked = useRef(false);
  // When the owner pressed the button: until the round asked for has ended, the button says it is starting.
  const [pressed, setPressed] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const stepping = status?.stepping === w.id;
  const can =
    status !== null &&
    status.mode === 'hold' &&
    status.halt === null &&
    status.stepping === null &&
    // A work that waits is asked too: its next round says what it waits for, or, once that came, is offered.
    (liveState(w, status) === 'ready' || w.state === 'waiting');
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
  // A judgment that waits offers no round: what it waits for, and where the owner gives it.
  if (n !== undefined && n.waits !== null) {
    return (
      <section className="next">
        <div className="next-head">
          <strong>{t('next_title')}</strong>
          <span>
            <span className="cell-id">{n.cell}</span> {n.title}
          </span>
        </div>
        <p className="prose">{t('next_waits', { what: n.waits })}</p>
        {n.design_card !== null ? (
          <div>
            <button type="button" className="decide-button" onClick={() => onCard(n.design_card ?? 0)}>
              {t('next_rule_card', { id: n.design_card })}
            </button>
          </div>
        ) : null}
        {n.ask !== null ? (
          <div>
            <button type="button" className="decide-button" onClick={() => onAsk(n.ask ?? 0)}>
              {t('next_answer_ask', { id: n.ask })}
            </button>
          </div>
        ) : null}
      </section>
    );
  }
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

/// The link to a round, which opens it in the panel.
function RoundLink({ round, onRound }: { round: number; onRound: (r: number) => void }) {
  return (
    <button type="button" className="link" onClick={() => onRound(round)}>
      {t('round_title', { id: round })}
    </button>
  );
}

/// The work's design cards: the one waiting for the owner's ruling first, with its controls; then the others, newest
/// first: a replaced one as what replaced it, a ruled one folded to its one line.
function Cards({ cards, onRound }: { cards: DesignCard[]; onRound: (r: number) => void }) {
  if (cards.length === 0) return <p className="muted">{t('work_no_design_cards')}</p>;
  const open = cards.filter((c) => c.status === 'open');
  const rest = cards.filter((c) => c.status !== 'open').reverse();
  const head = (c: DesignCard) => (
    <span className="work-item-head">
      <strong>{t('design_card_head', { id: c.id, status: words(c.status) })}</strong>
      <RoundLink round={c.round} onRound={onRound} />
    </span>
  );
  return (
    <>
      {[...open, ...rest].map((c) =>
        c.status === 'open' || c.status === 'replaced' ? (
          <div key={c.id} id={`design-card-${c.id}`} className="boxed">
            {head(c)}
            <DesignCardView c={c} />
          </div>
        ) : (
          <details key={c.id} id={`design-card-${c.id}`} className="boxed work-old">
            <summary>{head(c)}</summary>
            <DesignCardView c={c} />
          </details>
        ),
      )}
    </>
  );
}

/// Who told the owner a message: the engine, or an agent by its line and cell.
function teller(m: OwnerMessage): string {
  if (m.engine) return t('message_engine');
  return [m.line, m.cell !== null ? t('message_cell', { cell: m.cell }) : null].filter((x) => x !== null).join(' · ');
}

/// One thing the work's agents or the engine told the owner: who and when, the round it came with, the text; one
/// unread is marked read here as on the messages view.
function Told({ m, onRound, onAsk }: { m: OwnerMessage; onRound: (r: number) => void; onAsk: (id: number) => void }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <div className={m.read_at === null ? 'msg msg-unread' : 'msg'}>
      <div className="msg-head">
        <span className="muted">{teller(m)}</span>
        {m.at !== null ? <span className="muted">{time(m.at)}</span> : null}
        {m.round !== null ? <RoundLink round={m.round} onRound={onRound} /> : null}
        {m.read_at === null ? (
          <button
            type="button"
            className="link"
            onClick={() => {
              markRead(m.engine ? { notices: [m.id] } : { ids: [m.id] }).catch((e: unknown) =>
                setError(e instanceof Error ? e.message : String(e)),
              );
            }}
          >
            {t('message_mark_read')}
          </button>
        ) : null}
      </div>
      <p className="msg-text">{m.text}</p>
      {m.ask !== null ? (
        <button type="button" className="link" onClick={() => onAsk(m.ask ?? 0)}>
          {t('message_answer_ask', { id: m.ask })}
        </button>
      ) : null}
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// The work's messages: the asks waiting for the owner's answer first, with their forms; then what its agents and the
/// engine told the owner, newest first; the asks answered and the interjections folded under them.
function Messages({ d, onRound, onAsk }: { d: WorkDetail; onRound: (r: number) => void; onAsk: (id: number) => void }) {
  if (d.asks.length === 0 && d.messages.length === 0 && d.interjections.length === 0) {
    return <p className="muted">{t('messages_none')}</p>;
  }
  const waiting = d.asks.filter((a) => a.answer === null);
  const answered = d.asks.filter((a) => a.answer !== null).reverse();
  const head = (a: OwnerAsk) => (
    <span className="work-item-head">
      <strong>{t('ask_head', { id: a.id })}</strong>
      <span className="muted">{a.answer === null ? t('ask_waits') : t('ask_answered')}</span>
      <RoundLink round={a.round} onRound={onRound} />
    </span>
  );
  return (
    <>
      {waiting.map((a) => (
        <div key={a.id} id={`ask-${a.id}`} className="boxed">
          {head(a)}
          <AskView a={a} />
        </div>
      ))}
      {[...d.messages].reverse().map((m) => (
        <Told key={`${m.engine ? 'e' : 'a'}${m.id}`} m={m} onRound={onRound} onAsk={onAsk} />
      ))}
      {answered.length > 0 ? (
        <details className="boxed work-old">
          <summary>{t('work_answered_asks', { n: answered.length })}</summary>
          {answered.map((a) => (
            <div key={a.id} id={`ask-${a.id}`} className="boxed">
              {head(a)}
              <AskView a={a} />
            </div>
          ))}
        </details>
      ) : null}
      {d.interjections.length > 0 ? (
        <details className="boxed work-old">
          <summary>
            {t('interjections')} ({d.interjections.length})
          </summary>
          {d.interjections.map((x) => (
            <div key={x.id} className="boxed">
              <div className="muted">
                #{x.id} {x.speaker} → {x.line} ·{' '}
                {x.delivered_round !== null ? t('delivered', { round: x.delivered_round }) : t('waiting_delivery')}
              </div>
              <pre className="j-text">{x.text}</pre>
            </div>
          ))}
        </details>
      ) : null}
    </>
  );
}

const DRAWER_KEY = 'wf-agents.drawer-width';
const DRAWER_MIN = 300;

/// The window over the left of the chart: its tabs, the rounds (under why the work paused and its next round), the
/// design cards and the messages; its width drags by its right edge.
function Drawer({ d, g, r, go }: {
  d: WorkDetail;
  g: CompiledGraph;
  r: WorkRoute;
  go: (to: Partial<WorkRoute>) => void;
}) {
  const status = useStatus();
  const [open, setOpen] = useState(true);
  const [width, setWidth] = useState<number | null>(() => savedWidth(DRAWER_KEY, DRAWER_MIN));
  // What to bring into view once its tab is shown: the design card or the ask the owner was sent to; else a tab shows
  // from its top.
  const want = useRef<string | null>(null);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (want.current === null) body.current?.scrollTo({ top: 0 });
  }, [r.tab]);
  useEffect(() => {
    if (want.current !== null && bringIntoView(want.current)) want.current = null;
  });
  const show = (tab: WorkTab, at: string | null) => {
    want.current = at;
    setOpen(true);
    go({ tab });
  };
  const onRound = (round: number) => go({ round, cell: null });
  const onCard = (id: number) => show('cards', `design-card-${id}`);
  const onAsk = (id: number) => show('messages', `ask-${id}`);
  const openCards = d.design_cards.filter((c) => c.status === 'open').length;
  const openAsks = d.asks.filter((a) => a.answer === null).length;
  const tabs: Array<{ tab: WorkTab; label: string; n: number; chip: string | null; hint: string }> = [
    { tab: 'rounds', label: t('rounds'), n: d.rounds.length, chip: null, hint: '' },
    {
      tab: 'cards',
      label: t('design_cards'),
      n: d.design_cards.length,
      chip: openCards > 0 ? t('open_design_cards_chip', { n: openCards }) : null,
      hint: t('open_design_cards_n', { n: openCards }),
    },
    {
      tab: 'messages',
      label: t('work_messages_tab'),
      n: d.messages.length + d.asks.length,
      chip: openAsks > 0 ? t('open_asks_chip', { n: openAsks }) : null,
      hint: t('open_asks_n', { n: openAsks }),
    },
  ];
  const shown = roundsShown(d.rounds, r.only);
  const style = open && width !== null ? { width: `min(${width}px, calc(100vw - 32px))` } : undefined;
  return (
    <div className={`drawer${open ? '' : ' drawer-closed'}`} style={style}>
      <div className="drawer-head">
        <button
          type="button"
          className="drawer-toggle"
          onClick={() => setOpen((o) => !o)}
          aria-expanded={open}
          aria-label={t(open ? 'drawer_fold' : 'drawer_unfold')}
          title={t(open ? 'drawer_fold' : 'drawer_unfold')}
        >
          {open ? '‹' : '›'}
        </button>
        <div className="tabs" role="tablist">
          {tabs
            .filter((x) => open || x.tab === r.tab)
            .map((x) => (
              <button
                key={x.tab}
                type="button"
                role="tab"
                className="tab"
                aria-selected={x.tab === r.tab}
                onClick={() => show(x.tab, null)}
              >
                {x.label}
                <span className="tab-n">{x.n}</span>
                {x.chip !== null ? (
                  <span className="chip warn" title={x.hint}>
                    {x.chip}
                  </span>
                ) : null}
              </button>
            ))}
        </div>
      </div>
      {open ? (
        <div className="drawer-body" role="tabpanel" ref={body}>
          {r.tab === 'rounds' ? (
            <>
              {d.work.paused !== null ? <Paused d={d} /> : null}
              <Next d={d} onRound={onRound} onCard={onCard} onAsk={onAsk} />
              {r.only !== null ? (
                <div className="rounds-only">
                  <span>
                    {t(r.only === 'sent_back' ? 'rounds_only_sent_back' : 'rounds_only_failed', { n: shown.length })}
                  </span>
                  <button type="button" className="link" onClick={() => go({ only: null })}>
                    {t('rounds_show_all', { n: d.rounds.length })}
                  </button>
                </div>
              ) : null}
              <Timeline
                rounds={shown}
                g={g}
                stale={status !== null && status.mode === 'hold' && status.stepping !== d.work.id}
                selected={r.round}
                onRound={(x) => onRound(x.id)}
              />
            </>
          ) : null}
          {r.tab === 'cards' ? <Cards cards={d.design_cards} onRound={onRound} /> : null}
          {r.tab === 'messages' ? <Messages d={d} onRound={onRound} onAsk={onAsk} /> : null}
        </div>
      ) : null}
      {open ? (
        <WidthGrip side="right" min={DRAWER_MIN} storage={DRAWER_KEY} label={t('drawer_grip')} onWidth={setWidth} />
      ) : null}
    </div>
  );
}

function Body({ r, d, compiled, view }: { r: WorkRoute; d: WorkDetail; compiled: Compiled; view: FlowView }) {
  const g = compiled.graphs.find((x) => x.name === d.work.graph);
  const v = view.graphs.find((x) => x.name === d.work.graph);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const roundCell = r.round !== null ? (d.rounds.find((x) => x.id === r.round)?.cell ?? null) : null;
  const go = (to: Partial<WorkRoute>) => navigate(href({ ...r, ...to }), true);
  const model = chart(g, v);
  return (
    <>
      <Drawer d={d} g={g} r={r} go={go} />
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
        cell={r.cell}
        onCell={(c) => go({ round: null, cell: c })}
        start={roundCell ?? d.work.at}
        rounds={d.rounds}
        round={r.round}
        onRound={(x) => go({ round: x, cell: null })}
        renderRound={(x) => <RoundView id={x} rounds={d.rounds} />}
      />
    </>
  );
}

export function Work({ r }: { r: WorkRoute }) {
  const detail = useResource<WorkDetail>(`work:${r.id}`, `/api/v1/works/${r.id}`);
  const version = detail.data?.work.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  return (
    <div className="screen">
      <Loaded r={detail}>
        {(d) => (
          <Loaded r={compiled}>
            {(c) => <Loaded r={view}>{(v) => <Body r={r} d={d} compiled={c} view={v} />}</Loaded>}
          </Loaded>
        )}
      </Loaded>
    </div>
  );
}
