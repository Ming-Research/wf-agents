// A work's page, under its line above the chart, which holds its facts and the owner's controls: the state machine of
// the flow version it is pinned to, filling the window but for the column docked at its right, with what the work did
// on it: the cell it is at ringed, each cell it visited with its rounds and the rounds sent back or failed, the arrows
// it took standing out, the cell a round runs at in a moving halo and the cell a work held back stands at marked. The
// column holds on top the work's panel in three tabs: its rounds, under how it goes on and, while it does not go on by
// itself, its next round with the Step that runs it, resuming its session or in a new one as the owner chooses, and
// shown as it would be sent so; its design cards, the one waiting for the owner's ruling first
// with its controls; and its messages, the asks waiting for the owner's answer first with their forms, then what its
// agents and the engine told the owner. A round or a cell opened shows in the column's lower part, its cell brought
// into view.

import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  Compiled,
  CompiledGraph,
  DesignCard,
  FlowView,
  GraphView,
  NextRound,
  OwnerAsk,
  OwnerMessage,
  RoundSummary,
  WorkDetail,
  WorkSummary,
} from '../api';
import { controls, type Course, course, heldName, Loaded, OutcomeBadge } from '../components';
import { held, runningWords, SessionChoice, sessionKey, takeoverWords, useNewSession, waitsWords } from '../course';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName, time } from '../format';
import { overlays, roundArrows, taken, chart } from '../graph/flowModel';
import { postJson } from '../http';
import { spent } from '../round';
import { useActivity, useResource, useStatus } from '../live';
import { href, Link, navigate, type RoundsOnly, type WorkRoute, type WorkTab, workHref } from '../route';
import { t } from '../text';
import { AskView, bringIntoView, currentCard, DesignCardView, words } from './Decide';
import { markRead } from './Messages';
import { RoundView } from './RoundView';
import { Usage } from './Usage';

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

/// Where an agent's round spent its cost, beside its cost in its row: a small bar of the shares read again from the
/// cache, newly cached and output, and its steps; the whole of it on hovering. None for a round no agent ran.
function SpentBar({ r }: { r: RoundSummary }) {
  if (r.tokens === null || r.line === null || r.not_asked !== null) return null;
  const s = spent(r.tokens, r.model);
  const sum = s.shares !== null ? s.shares.read + s.shares.written + s.shares.output : 0;
  const width = (x: number) => `${sum > 0 ? (100 * x) / sum : 0}%`;
  return (
    <span className="spent" title={s.parts.map((p) => p.whole).join(' · ')}>
      {s.shares !== null ? (
        <span className="spent-bar">
          <span className="spent-read" style={{ width: width(s.shares.read) }} />
          <span className="spent-new" style={{ width: width(s.shares.written) }} />
          <span className="spent-output" style={{ width: width(s.shares.output) }} />
        </span>
      ) : null}
      <span className="spent-steps">{s.steps}</span>
    </span>
  );
}

/// The key of the small bars beside an agent round's cost: what each colour is and what the number beside the bar is.
function SpentKey() {
  return (
    <p className="spent-key">
      <span>
        <i className="spent-dot spent-read" />
        {t('spent_key_read')}
      </span>
      <span>
        <i className="spent-dot spent-new" />
        {t('spent_key_new')}
      </span>
      <span>
        <i className="spent-dot spent-output" />
        {t('spent_key_output')}
      </span>
      <span>{t('spent_key_steps')}</span>
    </p>
  );
}

/// The rounds, newest first: the cell and who did its work, the row it took (a badge only for what is not the usual
/// course: sent back, failed, cut off, running), what an agent's round cost, where its cost went and how long it took.
/// The round chosen, however it was chosen, is scrolled into view.
export function Timeline(props: {
  work: number;
  rounds: RoundSummary[];
  g: CompiledGraph;
  /// Whether a round still open is one left over, not one running: the engine holds and runs nothing of this work.
  stale: boolean;
  selected: number | null;
  onRound: (r: RoundSummary) => void;
}) {
  const body = useRef<HTMLTableSectionElement>(null);
  const { selected } = props;
  useEffect(() => {
    if (selected !== null) body.current?.querySelector('tr.selected')?.scrollIntoView({ block: 'nearest' });
  }, [selected]);
  if (props.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  const newest = [...props.rounds].reverse();
  return (
    <>
      {props.rounds.some((r) => r.tokens !== null) ? <SpentKey /> : null}
      <table className="list timeline">
        <tbody ref={body}>
          {newest.map((r) => {
            const what = r.not_asked !== null ? t('round_not_asked', { need: words(r.not_asked) }) : (r.transition ?? r.why ?? '');
            return (
              <tr
                key={r.n}
                className={`clickable${r.n === props.selected ? ' selected' : ''}`}
                onClick={() => props.onRound(r)}
              >
                <td className="muted nowrap">{roundName(props.work, r.n)}</td>
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
                <td className="num muted nowrap">
                  <SpentBar r={r} />
                  {r.cost_usd !== null && r.line !== null ? cost(r.cost_usd) : ''}
                </td>
                <td className="num muted">{r.ended_at !== null ? duration(r.ended_at - r.started_at) : ''}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </>
  );
}

/// How the work goes on, at the top of its rounds: whether a round of it runs (how long, and what the agent is doing),
/// what holds it back and why in full, and what it waits for, with where the owner gives it; on an older flow version,
/// that it is taken over when it goes on. The owner's controls of it are in its line above the chart.
function Going({ d, onCard, onAsk }: {
  d: WorkDetail;
  /// Open the design card, or the ask, the work waits for, in its tab of the window.
  onCard: (id: number) => void;
  onAsk: (id: number) => void;
}) {
  const status = useStatus();
  const w = d.work;
  const c = course(w, status);
  const activity = useActivity(w.id);
  const [now, setNow] = useState(() => Date.now());
  const runs = c.running !== null;
  useEffect(() => {
    if (!runs) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [runs]);
  if (c.over) return null;
  const why = held(c, w);
  const since = w.running?.started_at ?? null;
  const head = runs
    ? runningWords(c)
    : why !== null
      ? heldName(c).name
      : c.waits !== null
        ? waitsWords(c.waits)
        : c.started
          ? t('course_ready_started')
          : t('state_ready');
  const waits = c.waits;
  return (
    <section className={`next going${c.held === 'engine' ? ' paused' : ''}`}>
      <div className="next-head">
        <strong>{head}</strong>
        {runs && since !== null ? <span className="muted">{duration(now - since)}</span> : null}
      </div>
      {(c.held === 'engine' || c.held === 'architect') && w.paused !== null ? <p className="prose">{w.paused}</p> : null}
      {c.held === 'you' && why !== t('course_paused') ? <p className="prose">{why}</p> : null}
      {c.held === 'all' && !runs ? <p className="muted">{t('course_note_all')}</p> : null}
      {c.releasing ? <p className="muted">{t('course_releasing_why')}</p> : null}
      {waits !== null && (runs || why !== null) ? <p className="prose">{waitsWords(waits)}</p> : null}
      {waits?.kind === 'ruling' ? (
        <div>
          <button type="button" className="decide-button" onClick={() => onCard(waits.design_card)}>
            {t('next_rule_card', { id: waits.design_card })}
          </button>
        </div>
      ) : null}
      {waits?.kind === 'answer' ? (
        <div>
          <button type="button" className="decide-button" onClick={() => onAsk(waits.ask)}>
            {t('next_answer_ask', { id: waits.ask })}
          </button>
        </div>
      ) : null}
      {waits?.kind === 'items' ? (
        <p>
          {waits.items.map((i) => (
            <Link key={i} to={`/items/${i}`} className="chip">
              #{i}
            </Link>
          ))}
        </p>
      ) : null}
      {!runs && waits === null && w.answered !== null ? (
        <p className="muted">{t('course_answered', { id: w.answered })}</p>
      ) : null}
      {c.outdated ? <p className="warn">{takeoverWords(w, status)}</p> : null}
      {runs ? (
        <ol className="next-activity">
          {activity.map((x, i) => (
            <li key={i}>{x.what}</li>
          ))}
        </ol>
      ) : null}
    </section>
  );
}

/// The work's next round while it does not go on by itself and waits for nothing: what it will be, and the Step that
/// runs exactly that round; when the round ends, it opens, in the work that took this one over when it was on an older
/// flow version. A work on an older flow version has no next round of its own: Step takes it over. The architect's
/// wake is stepped by the architect's own Step.
export function Next({ d, onRound, onCard, onAsk, stepPath }: {
  d: WorkDetail;
  onRound: (r: number) => void;
  onCard: (id: number) => void;
  onAsk: (id: number) => void;
  stepPath?: string;
}) {
  const status = useStatus();
  const w = d.work;
  const c = course(w, status);
  const can = controls(c, status);
  const asked = useRef(false);
  // When the owner pressed Step: until the round asked for has ended, the button says it is starting.
  const [pressed, setPressed] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [newSession, setNewSession] = useNewSession(sessionKey(w));
  const preview = can.step && !c.outdated;
  const next = useResource<NextRound>(
    `work:${w.id}`,
    preview ? `/api/v1/works/${w.id}/next${newSession ? '?new_session=true' : ''}` : null,
  );
  const stepping = status?.stepping === w.id;
  // The round asked for has ended: open it, where it ran.
  const last = status?.last_step;
  useEffect(() => {
    if (!asked.current || stepping || !last || last.asked !== w.id) return;
    asked.current = false;
    const r = last.rounds[last.rounds.length - 1] ?? null;
    if (last.work !== w.id) navigate(workHref(last.work, { round: r }));
    else if (r !== null) onRound(r);
  }, [stepping, last, w.id, onRound]);
  if (!can.step) return null;
  const starting =
    pressed !== null && !(last !== null && last !== undefined && last.asked === w.id && last.at >= pressed);
  const run = () => {
    setError(null);
    setPressed(Date.now());
    asked.current = true;
    postJson(stepPath ?? `/api/v1/works/${w.id}/step`, { new_session: newSession }).then(
      () => setNewSession(false),
      (e: unknown) => {
        asked.current = false;
        setPressed(null);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  const button = (note: string, ready: boolean) => (
    <div className="next-run">
      <button
        type="button"
        className="next-button"
        onClick={run}
        disabled={!ready || starting || !can.canStep}
        title={can.canStep ? undefined : t('work_step_later')}
      >
        {starting ? t('next_starting') : t('work_step')}
      </button>
      <SessionChoice on={newSession} set={setNewSession} disabled={starting} />
      <span className="muted">{note}</span>
    </div>
  );
  if (c.outdated) {
    return (
      <section className="next">
        <div className="next-head">
          <strong>{t('next_title')}</strong>
        </div>
        {button(t('step_takeover_note', { now: status?.flow_version ?? '' }), true)}
        {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
      </section>
    );
  }
  const n = next.data;
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
      {button(note, n !== undefined)}
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </section>
  );
}

/// The link to a round of the work, by its number in the work, which opens it in the panel.
function RoundLink({ work, round, onRound }: { work: number; round: number; onRound: (r: number) => void }) {
  return (
    <button type="button" className="link" onClick={() => onRound(round)}>
      {roundName(work, round)}
    </button>
  );
}

/// The work's design cards, in the card every card shares: the one waiting for the owner's ruling first, then the
/// others, newest first; the current one (the one waiting, else the newest not replaced) open, every other folded to
/// its one line.
function Cards({ cards, onRound }: { cards: DesignCard[]; onRound: (r: number) => void }) {
  if (cards.length === 0) return <p className="muted">{t('work_no_design_cards')}</p>;
  const current = currentCard(cards);
  const open = cards.filter((c) => c.status === 'open');
  const rest = cards.filter((c) => c.status !== 'open').reverse();
  return (
    <>
      <p className="dc-lead">{t('dc_lead')}</p>
      {[...open, ...rest].map((c) => (
        <DesignCardView
          key={c.id}
          c={c}
          round={<RoundLink work={c.work} round={c.round} onRound={onRound} />}
          folded={c.id !== current}
        />
      ))}
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
        {m.round !== null && m.work !== null ? <RoundLink work={m.work} round={m.round} onRound={onRound} /> : null}
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
  const round = (a: OwnerAsk) => <RoundLink work={a.work} round={a.round} onRound={onRound} />;
  return (
    <>
      {waiting.map((a) => (
        <AskView key={a.id} a={a} round={round(a)} />
      ))}
      {[...d.messages].reverse().map((m) => (
        <Told key={`${m.engine ? 'e' : 'a'}${m.id}`} m={m} onRound={onRound} onAsk={onAsk} />
      ))}
      {answered.length > 0 ? (
        <details className="boxed work-old">
          <summary>{t('work_answered_asks', { n: answered.length })}</summary>
          {answered.map((a) => (
            <AskView key={a.id} a={a} round={round(a)} />
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
                {x.delivered_round !== null ? t('delivered', { round: roundName(x.work, x.delivered_round) }) : t('waiting_delivery')}
              </div>
              <pre className="j-text">{x.text}</pre>
            </div>
          ))}
        </details>
      ) : null}
    </>
  );
}

/// The work's panel, on top of the column docked at the right of its chart: its tabs, the rounds (under how the work
/// goes on and its next round), the design cards and the messages.
function WorkPanel({ d, g, r, go }: {
  d: WorkDetail;
  g: CompiledGraph;
  r: WorkRoute;
  go: (to: Partial<WorkRoute>) => void;
}) {
  const status = useStatus();
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
    go({ tab });
  };
  const onRound = (round: number) => go({ round, cell: null });
  const onCard = (id: number) => show('cards', `design-card-${id}`);
  const onAsk = (id: number) => show('messages', `ask-${id}`);
  const openCards = d.design_cards.filter((c) => c.status === 'open').length;
  const openAsks = d.asks.filter((a) => a.answer === null).length;
  const tabs: Array<{ tab: WorkTab; label: string; n: number | null; chip: string | null; hint: string }> = [
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
    { tab: 'usage', label: t('us_tab'), n: null, chip: null, hint: '' },
  ];
  const shown = roundsShown(d.rounds, r.only);
  return (
    <div className="drawer">
      <div className="drawer-head">
        <div className="tabs" role="tablist">
          {tabs.map((x) => (
            <button
              key={x.tab}
              type="button"
              role="tab"
              className="tab"
              aria-selected={x.tab === r.tab}
              onClick={() => show(x.tab, null)}
            >
              {x.label}
              {x.n !== null ? <span className="tab-n">{x.n}</span> : null}
              {x.chip !== null ? (
                <span className="chip warn" title={x.hint}>
                  {x.chip}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      </div>
      <div className="drawer-body" role="tabpanel" ref={body}>
        {r.tab === 'rounds' ? (
          <>
            <Going d={d} onCard={onCard} onAsk={onAsk} />
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
              work={d.work.id}
              rounds={shown}
              g={g}
              stale={status !== null && status.running !== d.work.id}
              selected={r.round}
              onRound={(x) => onRound(x.n)}
            />
          </>
        ) : null}
        {r.tab === 'cards' ? <Cards cards={d.design_cards} onRound={onRound} /> : null}
        {r.tab === 'messages' ? <Messages d={d} onRound={onRound} onAsk={onAsk} /> : null}
        {r.tab === 'usage' ? <Usage d={d} g={g} /> : null}
      </div>
    </div>
  );
}

/// What a work's chart marks: what it did at each cell (the cell it is at, whether a round runs there and why it is
/// held back), how many times it took each arrow, and the arrows the round chosen came in by and left by.
export function workMarks(
  g: CompiledGraph,
  v: GraphView,
  w: WorkSummary,
  rounds: RoundSummary[],
  c: Course,
  round: number | null = null,
) {
  const model = chart(g, v);
  return {
    overlays: overlays(rounds, w.at, { running: c.running !== null, held: held(c, w) }),
    taken: taken(model, g, rounds),
    lit: round !== null ? roundArrows(model, g, rounds, round) : undefined,
    added: new Set<string>(),
    changed: new Set<string>(),
  };
}

function Body({ r, d, compiled, view }: { r: WorkRoute; d: WorkDetail; compiled: Compiled; view: FlowView }) {
  const status = useStatus();
  const g = compiled.graphs.find((x) => x.name === d.work.graph);
  const v = view.graphs.find((x) => x.name === d.work.graph);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const roundCell = r.round !== null ? (d.rounds.find((x) => x.n === r.round)?.cell ?? null) : null;
  const go = (to: Partial<WorkRoute>) => navigate(href({ ...r, ...to }), true);
  const c = course(d.work, status);
  return (
    <FlowScreen
      compiled={compiled}
      graphs={graphs}
      legend="work"
      marks={[workMarks(g, v, d.work, d.rounds, c, r.round)]}
      cell={r.cell}
      onCell={(c) => go({ round: null, cell: c })}
      start={roundCell ?? d.work.at}
      work={d.work.id}
      rounds={d.rounds}
      round={r.round}
      onRound={(x) => go({ round: x, cell: null })}
      renderRound={(x) => <RoundView work={d.work.id} n={x} rounds={d.rounds} />}
      side={<WorkPanel d={d} g={g} r={r} go={go} />}
    />
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
