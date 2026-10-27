// A work, for diagnosis: the state machine of the flow version it is pinned to with its path over it, its rounds,
// and a round in full. A round leads to its cell on the chart, and a cell to its rounds.

import type { Compiled, CompiledGraph, RoundSummary, WorkDetail } from '../api';
import { Facts, Json, Loaded, OutcomeBadge, Section, WorkBadge } from '../components';
import { cost, duration, time } from '../format';
import { FlowChart } from '../graph/FlowChart';
import { useResource } from '../live';
import { href, Link, navigate } from '../route';
import { useT } from '../text';
import { CellView } from './CellView';
import { RoundView } from './RoundView';

function Timeline(props: {
  rounds: RoundSummary[];
  selected: number | null;
  onRound: (r: RoundSummary) => void;
}) {
  const t = useT();
  if (props.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  const newest = [...props.rounds].reverse();
  return (
    <table className="list timeline">
      <tbody>
        {newest.map((r) => (
          <tr
            key={r.id}
            className={`clickable${r.id === props.selected ? ' selected' : ''}`}
            onClick={() => props.onRound(r)}
          >
            <td className="muted">#{r.id}</td>
            <td>
              <span className="cell-id">{r.cell ?? '–'}</span>
              {r.line ? <span className="muted"> {r.line}</span> : null}
            </td>
            <td>
              <OutcomeBadge outcome={r.outcome} />
            </td>
            <td className="clip" title={r.transition ?? r.why ?? ''}>
              {r.transition ?? r.why ?? ''}
            </td>
            <td className="muted">{time(r.started_at)}</td>
            <td className="num muted">{r.ended_at !== null ? duration(r.ended_at - r.started_at) : ''}</td>
            <td className="num muted">{cost(r.cost_usd)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Header({ d }: { d: WorkDetail }) {
  const t = useT();
  const w = d.work;
  return (
    <header className="work-head">
      <h1>
        {t('work_title', { id: w.id })}
        {w.item !== null ? (
          <Link to={`/items/${w.item}`} className="head-item">
            #{w.item} {w.title}
          </Link>
        ) : null}
        <WorkBadge state={w.state} />
      </h1>
      <div className="head-facts">
        <span>
          <span className="cell-id">{w.at}</span> {w.at_title}
        </span>
        {w.stage ? <span>{w.stage}</span> : null}
        <Link to={`/flow/${w.flow_version}`}>{t('work_flow', { version: w.flow_version, graph: w.graph })}</Link>
        <span>
          {t('col_rounds')} {w.rounds}
        </span>
        {w.sent_back > 0 ? <span className="warn">{t('sent_back_n', { n: w.sent_back })}</span> : null}
        {w.failed > 0 ? <span className="bad">{t('failed_n', { n: w.failed })}</span> : null}
        {w.open_cards > 0 ? <span className="warn">{t('open_cards_n', { n: w.open_cards })}</span> : null}
        <span>{cost(w.cost_usd)}</span>
        {w.pr !== null ? <span>PR #{w.pr}</span> : null}
      </div>
      {w.paused ? <div className="why">{t('paused_why', { why: w.paused })}</div> : null}
      {w.ended ? <div className="why">{t('ended_why', { why: w.ended })}</div> : null}
    </header>
  );
}

function Body({ id, d, graph, round, cell }: {
  id: number;
  d: WorkDetail;
  graph: CompiledGraph | null;
  round: number | null;
  cell: string | null;
}) {
  const t = useT();
  const roundCell = round !== null ? (d.rounds.find((r) => r.id === round)?.cell ?? null) : null;
  const selectedCell = cell ?? roundCell;
  const shown = cell !== null ? d.rounds.filter((r) => r.cell === cell) : d.rounds;
  const cellDef = selectedCell !== null && graph ? (graph.cells.find((c) => c.id === selectedCell) ?? null) : null;
  const go = (r: number | null, c: string | null) => navigate(href({ page: 'work', id, round: r, cell: c }));
  return (
    <div className="split">
      <div className="split-chart">
        {graph ? (
          <FlowChart
            graph={graph}
            rounds={d.rounds}
            at={d.work.at}
            selected={selectedCell}
            focus={roundCell ?? d.work.at}
            onSelect={(c) => go(null, c)}
          />
        ) : null}
      </div>
      <div className="split-side">
        {round !== null ? <RoundView id={round} onClose={() => go(null, cell)} /> : null}
        <Section
          title={cell !== null ? t('rounds_at', { cell }) : t('rounds')}
          extra={
            cell !== null ? (
              <button
                className="link"
                onClick={(e) => {
                  e.preventDefault();
                  go(round, null);
                }}
              >
                {t('show_all')}
              </button>
            ) : null
          }
        >
          <Timeline rounds={shown} selected={round} onRound={(r) => go(r.id, cell)} />
        </Section>
        {cellDef ? (
          <Section title={`${t('definition')}: ${cellDef.id} ${cellDef.title}`} open={false}>
            <CellView cell={cellDef} />
          </Section>
        ) : null}
        {d.cards.length > 0 ? (
          <Section title={t('cards')} open={false}>
            {d.cards.map((c) => (
              <div key={c.id} className="card-box">
                <div>
                  #{c.id} {c.kind} · {c.status} ·{' '}
                  <button className="link" onClick={() => go(c.round, null)}>
                    {t('round_title', { id: c.round })}
                  </button>
                </div>
                <Json value={c.body} />
                <Json value={c.rulings} />
              </div>
            ))}
          </Section>
        ) : null}
        {d.interjections.length > 0 ? (
          <Section title={t('interjections')} open={false}>
            {d.interjections.map((x) => (
              <div key={x.id} className="card-box">
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
        <Section title={t('recorded')} open={false}>
          <Facts
            rows={[
              [t('machine'), d.machine],
              [t('worktree'), d.worktree],
              [t('branch'), d.work.branch],
              [t('pushed_head'), d.pushed_head],
              [t('resume_at'), d.resume_at],
              [t('waiting_cells'), <Json key="w" value={d.waiting} />],
              [t('recorded'), <Json key="r" value={d.recorded} />],
            ]}
          />
        </Section>
      </div>
    </div>
  );
}

export function Work({ id, round, cell }: { id: number; round: number | null; cell: string | null }) {
  const detail = useResource<WorkDetail>(`work:${id}`, `/api/v1/works/${id}`);
  const version = detail.data?.work.flow_version ?? null;
  const flow = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  return (
    <div className="page page-wide">
      <Loaded r={detail}>
        {(d) => {
          const graph = flow.data?.graphs.find((g) => g.name === d.work.graph) ?? null;
          return (
            <>
              <Header d={d} />
              <Body id={id} d={d} graph={graph} round={round} cell={cell} />
            </>
          );
        }}
      </Loaded>
    </div>
  );
}
