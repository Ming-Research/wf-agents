// The System view: how the whole system stands, in five parts, each for what the owner does with it. Now says whether
// the owner needs to step in: what waits for the owner, the works by where they stand, and each project's architect.
// Output says what got done, Spend where the money goes, Waste and efficiency where to save, and Anomalies what went
// wrong, each over the period the owner picks: the week under way, the last four weeks, or all time. Each part sums up
// first, in tiles; a tile's number leads to the list it counts, folded under the tiles or on a view of its own; a trend
// is drawn, to scale, where the period holds more than one week.

import { Flash, Link as PrimerLink } from '@primer/react';

import type {
  Anomaly,
  ArchitectNow,
  ItemAt,
  Status,
  System as SystemData,
  Week,
  WorkCount,
  WorkSummary,
} from '../api';
import { Columns, share, Tile, useTip } from '../charts';
import { course, heldWords } from '../components';
import { runningWords, waitsWords } from '../course';
import { cost, day, duration, roundName, time } from '../format';
import { useResource, useStatus } from '../live';
import { HeadingLevel, ItemRef, RoundLink, Section } from '../parts';
import { architectHref, href, Link, type Period, PERIODS, workHref } from '../route';
import { tokenCount, tokens } from '../round';
import { spendTable } from '../spend';
import { anomaliesBy, byStanding, mergedAverages, spentIn, systemPath, zone } from '../system';
import { t, tn } from '../text';
import { modelName } from './RoundView';

export function System({ period }: { period: Period }) {
  const status = useStatus();
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const r = useResource<SystemData>('system', systemPath(period, zone()));
  const tip = useTip();
  if (r.error !== null) {
    return (
      <div className="page">
        <Flash variant="danger">{t('error', { error: r.error })}</Flash>
      </div>
    );
  }
  if (r.data === undefined) return <p className="page muted">{t('loading')}</p>;
  const s = r.data;
  const part = (title: string, purpose: string) => ({
    title,
    by: <span className="sys-for">{purpose}</span>,
  });
  return (
    <div className="page sys" onMouseMove={tip.move} onMouseLeave={tip.leave}>
      {/* The view's five parts are its own sections, as the UI spec heads a page's sections. */}
      <HeadingLevel.Provider value={2}>
        <Section {...part(t('sys_now'), t('sys_now_for'))} open>
          <Now s={s} works={works.data ?? []} status={status} />
        </Section>
        <PeriodSwitch period={period} />
        <Section {...part(t('sys_output'), t('sys_output_for'))} open>
          <Output s={s} status={status} />
        </Section>
        <Section {...part(t('sys_spend'), t('sys_spend_for'))} open>
          <Spend s={s} />
        </Section>
        <Section {...part(t('sys_waste'), t('sys_waste_for'))} open>
          <Waste s={s} />
        </Section>
        <Section {...part(t('sys_anomalies'), t('sys_anomalies_for'))} open>
          <Anomalies s={s} />
        </Section>
      </HeadingLevel.Provider>
      {tip.shown}
    </div>
  );
}

/// The period the parts after Now count over, kept in the address.
function PeriodSwitch({ period }: { period: Period }) {
  return (
    <div className="sys-period">
      <span className="muted">{t('sys_period')}</span>
      {PERIODS.map((p) => (
        <Link key={p} to={href({ page: 'system', period: p })} className={p === period ? 'sys-period-on' : undefined}>
          {t(`sys_period_${p}`)}
        </Link>
      ))}
    </div>
  );
}

/// A number of nothing, said as such.
const none = () => t('sys_no_value');

/// Whether the owner needs to step in: what waits for the owner, with how long the oldest has waited, each leading to
/// the conversations it is in; the works by where they stand, each count opening its works; and each project's
/// architect.
function Now({ s, works, status }: { s: SystemData; works: WorkSummary[]; status: Status | null }) {
  const n = s.now;
  const groups = byStanding(works, status);
  const oldest = (at: number | null) => (at !== null ? t('sys_oldest', { for: duration(s.at - at) }) : undefined);
  return (
    <>
      <div className="us-tiles">
        <Tile k={t('sys_asks')} v={String(n.asks.n)} s={oldest(n.asks.oldest)} to="/messages" none={n.asks.n === 0} />
        <Tile
          k={t('sys_cards')}
          v={String(n.design_cards.n)}
          s={oldest(n.design_cards.oldest)}
          to="/messages"
          none={n.design_cards.n === 0}
        />
        <Tile
          k={t('sys_faults')}
          v={String(n.faults.n)}
          s={oldest(n.faults.oldest)}
          to={href({ page: 'messages', open: 'wf-agents' })}
          none={n.faults.n === 0}
        />
      </div>
      <div className="us-tiles">
        {groups.map((g) => (
          <Tile
            key={g.standing}
            k={t(`sys_works_${g.standing}`)}
            v={String(g.works.length)}
            open={`sys-works-${g.standing}`}
            none={g.works.length === 0}
            hint={t(`sys_works_${g.standing}_hint`)}
          />
        ))}
      </div>
      <Architects list={n.architects} at={s.at} />
      {groups
        .filter((g) => g.works.length > 0)
        .map((g) => (
          <Section
            key={g.standing}
            id={`sys-works-${g.standing}`}
            title={t(`sys_works_${g.standing}`)}
            open={g.standing === 'you' || g.standing === 'stopped'}
          >
            <WorkRows list={g.works} status={status} at={s.at} />
          </Section>
        ))}
    </>
  );
}

/// How a work stands, in the works list's words: a round of it running, the engine's stop, what it waits for, or what
/// else holds it back; nothing for a work only every work's pause holds, which the bar says.
function standingWords(w: WorkSummary, status: Status | null): string {
  const c = course(w, status);
  if (c.running !== null) return runningWords(c);
  if (c.held === 'engine') return heldWords(c, w.paused);
  if (c.waits !== null) return waitsWords(c.waits);
  if (c.held !== null && c.held !== 'all') return heldWords(c, w.paused);
  return '';
}

/// Works, each by its number and item, how it stands, and how long since its last round.
function WorkRows({ list, status, at }: { list: WorkSummary[]; status: Status | null; at: number }) {
  return (
    <table className="list us-table sys-table">
      <thead>
        <tr>
          <th>{t('sys_col_work')}</th>
          <th>{t('sys_col_item')}</th>
          <th>{t('sys_col_how')}</th>
          <th className="num">{t('sys_col_for')}</th>
        </tr>
      </thead>
      <tbody>
        {list.map((w) => (
          <tr key={w.id}>
            <td>
              <Link to={workHref(w.id)}>#{w.id}</Link>
            </td>
            <td>{w.item !== null ? <ItemRef id={w.item} title={w.title} /> : null}</td>
            <td>{standingWords(w, status)}</td>
            <td className="num">{w.last_at !== null ? duration(at - w.last_at) : ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// Each project's architect: working, paused or idle, with no word of its works; how many matters wait in its queue,
/// which its one-to-one holds, and how long the oldest has waited.
function Architects({ list, at }: { list: ArchitectNow[]; at: number }) {
  if (list.length === 0) return null;
  return (
    <table className="list us-table sys-table">
      <thead>
        <tr>
          <th>{t('sys_col_architect')}</th>
          <th>{t('sys_col_now')}</th>
          <th className="num">{t('sys_col_queue')}</th>
          <th className="num">{t('sys_col_oldest')}</th>
        </tr>
      </thead>
      <tbody>
        {list.map((a) => (
          <tr key={a.project}>
            <td>
              <Link to={architectHref({ project: a.project })}>{a.project}</Link>
            </td>
            <td>
              {a.work === null ? (
                t('sys_idle')
              ) : (
                <Link to={architectHref({ project: a.project, work: a.work })}>
                  {t(a.paused ? 'sys_working_paused' : 'sys_working', { work: a.work })}
                </Link>
              )}
            </td>
            <td className="num">
              {a.queue > 0 ? <Link to={href({ page: 'messages', open: `architect:${a.project}` })}>{a.queue}</Link> : 0}
            </td>
            <td className="num">{a.oldest !== null ? duration(at - a.oldest) : ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// What got done: the items merged, made and cancelled in the period and the pull requests open now, each opening its
/// list; how long a merged work took from its start, and what it cost; and, over more than one week, the same week by
/// week.
function Output({ s, status }: { s: SystemData; status: Status | null }) {
  const o = s.output;
  const avg = mergedAverages(o.merged);
  return (
    <>
      <div className="us-tiles">
        <Tile k={t('sys_merged')} v={String(o.merged.length)} open="sys-merged" none={o.merged.length === 0} />
        <Tile k={t('sys_made')} v={String(o.made.length)} open="sys-made" none={o.made.length === 0} />
        <Tile k={t('sys_cancelled')} v={String(o.cancelled.length)} open="sys-cancelled" none={o.cancelled.length === 0} />
        <Tile
          k={t('sys_open_prs')}
          v={String(o.open_prs.length)}
          s={t('sys_open_prs_s')}
          open="sys-prs"
          none={o.open_prs.length === 0}
        />
        <Tile
          k={t('sys_to_merge')}
          v={avg !== null ? duration(avg.took) : none()}
          s={t('sys_on_average')}
          none={avg === null}
          hint={t('sys_to_merge_hint')}
        />
      </div>
      {s.weeks.length > 1 ? <WeekTable weeks={s.weeks} /> : null}
      {o.merged.length > 0 ? (
        <Section id="sys-merged" title={t('sys_merged')}>
          <table className="list us-table sys-table">
            <thead>
              <tr>
                <th>{t('sys_col_item')}</th>
                <th>{t('sys_col_work')}</th>
                <th>{t('sys_col_merged')}</th>
                <th className="num">{t('sys_col_took')}</th>
                <th className="num">{t('sys_col_cost')}</th>
              </tr>
            </thead>
            <tbody>
              {o.merged.map((m) => (
                <tr key={m.work}>
                  <td>
                    <ItemRef id={m.item} title={m.title} />
                  </td>
                  <td>
                    <Link to={workHref(m.work)}>#{m.work}</Link>
                  </td>
                  <td>{time(m.merged_at)}</td>
                  <td className="num">{duration(m.merged_at - m.started_at)}</td>
                  <td className="num">{cost(m.cost_usd)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      ) : null}
      {o.made.length > 0 ? (
        <Section id="sys-made" title={t('sys_made')}>
          <ItemRows list={o.made} why={false} />
        </Section>
      ) : null}
      {o.cancelled.length > 0 ? (
        <Section id="sys-cancelled" title={t('sys_cancelled')}>
          <ItemRows list={o.cancelled} why />
        </Section>
      ) : null}
      {o.open_prs.length > 0 ? (
        <Section id="sys-prs" title={t('sys_open_prs')}>
          <table className="list us-table sys-table">
            <thead>
              <tr>
                <th>{t('sys_col_pr')}</th>
                <th>{t('sys_col_item')}</th>
                <th>{t('sys_col_work')}</th>
                <th className="num">{t('sys_col_open_for')}</th>
              </tr>
            </thead>
            <tbody>
              {o.open_prs.map((p) => {
                const repo = p.project !== null ? status?.repos[p.project] : undefined;
                return (
                  <tr key={`${p.project ?? ''}-${p.pr}`}>
                    <td>
                      {repo !== undefined ? (
                        <PrimerLink href={`https://github.com/${repo}/pull/${p.pr}`} target="_blank" rel="noreferrer">
                          {t('work_pr', { n: p.pr })}
                        </PrimerLink>
                      ) : (
                        t('work_pr', { n: p.pr })
                      )}
                    </td>
                    <td>{p.item !== null ? <ItemRef id={p.item} title={p.title} /> : null}</td>
                    <td>
                      <Link to={workHref(p.work)}>#{p.work}</Link>
                    </td>
                    <td className="num">{duration(s.at - p.since)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </Section>
      ) : null}
    </>
  );
}

/// Items made or cancelled, the latest first, each with when, and for one cancelled why.
function ItemRows({ list, why }: { list: ItemAt[]; why: boolean }) {
  return (
    <table className="list us-table sys-table">
      <thead>
        <tr>
          <th>{t('sys_col_item')}</th>
          <th>{t('sys_col_when')}</th>
          {why ? <th>{t('sys_col_why')}</th> : null}
        </tr>
      </thead>
      <tbody>
        {list.map((i) => (
          <tr key={i.item}>
            <td>
              <ItemRef id={i.item} title={i.title} />
            </td>
            <td>{time(i.at)}</td>
            {why ? <td>{i.why}</td> : null}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// What got done week by week, the latest first.
function WeekTable({ weeks }: { weeks: Week[] }) {
  return (
    <table className="list us-table sys-table sys-weeks">
      <thead>
        <tr>
          <th>{t('sys_col_week')}</th>
          <th className="num">{t('sys_merged')}</th>
          <th className="num">{t('sys_made')}</th>
          <th className="num">{t('sys_cancelled')}</th>
          <th className="num" title={t('sys_col_open_prs_hint')}>
            {t('sys_open_prs')}
          </th>
        </tr>
      </thead>
      <tbody>
        {[...weeks].reverse().map((w) => (
          <tr key={w.start}>
            <td>{day(w.start)}</td>
            <td className="num">{w.merged}</td>
            <td className="num">{w.made}</td>
            <td className="num">{w.cancelled}</td>
            <td className="num">{w.open_prs}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// Where the money goes: what the period spent, the engineers' works and the architects' works apart, and per item
/// merged; over more than one week, both week by week; then, folded, by line and model, by item, and the ten costliest
/// cells.
function Spend({ s }: { s: SystemData }) {
  const spent = spentIn(s.weeks);
  const merged = s.output.merged.length;
  const own = mergedAverages(s.output.merged);
  const lines = spendTable(s.spend.by_line_model);
  const engineers = { colour: 'var(--chart-1)', label: t('sys_engineers') };
  const architects = { colour: 'var(--chart-2)', label: t('sys_architects') };
  return (
    <>
      <div className="us-tiles">
        <Tile k={t('sys_spent')} v={cost(spent.total)} open="sys-by-line" none={spent.total === 0} />
        <Tile k={t('sys_engineers')} v={cost(spent.engineers)} open="sys-by-item" none={spent.engineers === 0} />
        <Tile k={t('sys_architects')} v={cost(spent.architects)} open="sys-by-line" none={spent.architects === 0} />
        <Tile
          k={t('sys_per_merged')}
          v={merged > 0 ? cost(spent.total / merged) : none()}
          s={own !== null ? tn('sys_per_merged_s', merged, { own: cost(own.cost) }) : undefined}
          none={merged === 0}
          hint={t('sys_per_merged_hint')}
        />
      </div>
      {s.weeks.length > 1 ? (
        <Columns
          columns={s.weeks.map((w) => ({
            label: day(w.start),
            parts: [
              {
                ...engineers,
                value: w.engineers_usd,
                tip: t('sys_week_tip', { week: day(w.start), who: engineers.label, cost: cost(w.engineers_usd) }),
              },
              {
                ...architects,
                value: w.architects_usd,
                tip: t('sys_week_tip', { week: day(w.start), who: architects.label, cost: cost(w.architects_usd) }),
              },
            ],
          }))}
          legend={[engineers, architects]}
          top={(max) => cost(max)}
        />
      ) : null}
      {lines.models.length > 0 ? (
        <Section id="sys-by-line" title={t('sys_by_line')}>
          <table className="list us-table sys-table">
            <thead>
              <tr>
                <th>{t('sys_col_line')}</th>
                {lines.models.map((m) => (
                  <th key={m} className="num">
                    {modelName(m)}
                  </th>
                ))}
                <th className="num">{t('sys_col_total')}</th>
              </tr>
            </thead>
            <tbody>
              {lines.lines.map((l) => (
                <tr key={l.key}>
                  <td>
                    {l.key}
                  </td>
                  {lines.models.map((m) => (
                    <td key={m} className="num">
                      {cost(l.byModel[m])}
                    </td>
                  ))}
                  <td className="num">
                    {cost(l.total)}
                  </td>
                </tr>
              ))}
              <tr>
                <td>{t('sys_col_total')}</td>
                {lines.models.map((m) => (
                  <td key={m} className="num">
                    {cost(lines.byModel[m])}
                  </td>
                ))}
                <td className="num">
                  {cost(lines.total)}
                </td>
              </tr>
            </tbody>
          </table>
        </Section>
      ) : null}
      {s.spend.by_item.length > 0 ? (
        <Section id="sys-by-item" title={t('sys_by_item')}>
          <table className="list us-table sys-table">
            <thead>
              <tr>
                <th>{t('sys_col_item')}</th>
                <th className="num">{t('sys_col_works')}</th>
                <th className="num">{t('sys_col_rounds')}</th>
                <th className="num">{t('sys_col_cost')}</th>
              </tr>
            </thead>
            <tbody>
              {s.spend.by_item.map((i) => (
                <tr key={i.item}>
                  <td>
                    <ItemRef id={i.item} title={i.title} />
                  </td>
                  <td className="num">{i.works}</td>
                  <td className="num">{i.rounds}</td>
                  <td className="num" data-tip={t('sys_share_tip', { share: share(i.cost_usd, spent.engineers) })}>
                    {cost(i.cost_usd)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      ) : null}
      {s.spend.cells.length > 0 ? (
        <Section id="sys-cells" title={t('sys_cells')}>
          <table className="list us-table sys-table">
            <thead>
              <tr>
                <th>{t('sys_col_cell')}</th>
                <th className="num">{t('sys_col_rounds')}</th>
                <th className="num">{t('sys_col_cost')}</th>
              </tr>
            </thead>
            <tbody>
              {s.spend.cells.map((c) => (
                <tr key={`${c.graph}-${c.cell}`}>
                  <td>
                    <Link
                      to={href({
                        page: 'flow',
                        version: null,
                        graph: c.graph,
                        cell: c.cell,
                        project: null,
                        ref: null,
                        from: null,
                      })}
                    >
                      <span className="cell-id">{c.cell}</span> {c.title ?? ''}
                    </Link>
                    {c.graph !== 'engineer' ? <span className="muted"> · {c.graph}</span> : null}
                  </td>
                  <td className="num">{c.rounds}</td>
                  <td className="num">{cost(c.cost_usd)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      ) : null}
    </>
  );
}

/// What a halt's cause is, in words.
function haltName(kind: string): string {
  if (kind === 'usage_limit') return t('sys_halt_usage_limit');
  if (kind === 'offline') return t('sys_halt_offline');
  if (kind === 'cannot_run') return t('sys_halt_cannot_run');
  return kind;
}

/// How many rounds or pings a list of works counts, and what they cost.
function summed(list: WorkCount[]): { n: number; cost: number } {
  return { n: list.reduce((a, c) => a + c.n, 0), cost: list.reduce((a, c) => a + c.cost_usd, 0) };
}

/// Where a work's rounds or pings are shown: an engineer's work's page as `engineer` names it, or the architect's
/// view, its rounds there, named by no work.
function workPlace(c: WorkCount, engineer: string): string {
  return c.graph === 'architect' ? architectHref({ project: c.project, work: c.work }) : engineer;
}

/// Where to save: how much of the agents' input the prompt cache gave, and the rounds that missed it; the rounds sent
/// back, and those that failed or were cut off, with what they cost; how long a failed run's cause halted every work;
/// and what the keep-warm pings cost. Each count opens its works.
function Waste({ s }: { s: SystemData }) {
  const w = s.waste;
  const given = w.read + w.written + w.input;
  const back = summed(w.sent_back);
  const failed = summed(w.failed);
  const pings = summed(w.pings);
  return (
    <>
      <div className="us-tiles">
        <Tile
          k={t('sys_cache')}
          v={given > 0 ? share(w.read, given) : none()}
          s={t('sys_cache_s', { written: tokens(w.written), input: tokens(w.input) })}
          none={given === 0}
          hint={t('sys_cache_hint')}
        />
        <Tile
          k={t('sys_misses')}
          v={String(w.misses)}
          s={t('sys_misses_s', { cost: cost(w.miss_usd), tokens: tokenCount(w.missed) })}
          to={href({ page: 'messages', open: 'wf-agents' })}
          none={w.misses === 0}
          hint={t('sys_misses_hint')}
        />
        <Tile k={t('sys_sent_back')} v={String(back.n)} s={cost(back.cost)} open="sys-sent-back" none={back.n === 0} />
        <Tile k={t('sys_failed')} v={String(failed.n)} s={cost(failed.cost)} open="sys-failed" none={failed.n === 0} />
        <Tile
          k={t('sys_halted')}
          v={duration(w.halted_ms)}
          s={w.halts.map((h) => `${haltName(h.kind)} ${duration(h.ms)}`).join(' · ')}
          none={w.halted_ms === 0}
          hint={t('sys_halted_hint')}
        />
        <Tile k={t('sys_pings')} v={String(pings.n)} s={cost(pings.cost)} open="sys-pings" none={pings.n === 0} />
      </div>
      {w.sent_back.length > 0 ? (
        <Section id="sys-sent-back" title={t('sys_sent_back_by_work')}>
          <Counts
            list={w.sent_back}
            head={t('sys_col_rounds')}
            to={(c) => workPlace(c, workHref(c.work, { only: 'sent_back' }))}
          />
        </Section>
      ) : null}
      {w.failed.length > 0 ? (
        <Section id="sys-failed" title={t('sys_failed_by_work')}>
          <Counts list={w.failed} head={t('sys_col_rounds')} to={(c) => workPlace(c, workHref(c.work, { only: 'failed' }))} />
        </Section>
      ) : null}
      {w.pings.length > 0 ? (
        <Section id="sys-pings" title={t('sys_pings_by_work')}>
          <Counts list={w.pings} head={t('sys_col_pings')} to={(c) => workPlace(c, workHref(c.work, { tab: 'usage' }))} />
        </Section>
      ) : null}
    </>
  );
}

/// Works, each with how many of its rounds or pings and what they cost, the costliest first, each leading to where the
/// work shows them.
function Counts({ list, head, to }: { list: WorkCount[]; head: string; to: (c: WorkCount) => string }) {
  return (
    <table className="list us-table sys-table">
      <thead>
        <tr>
          <th>{t('sys_col_work')}</th>
          <th>{t('sys_col_item')}</th>
          <th className="num">{head}</th>
          <th className="num">{t('sys_col_cost')}</th>
        </tr>
      </thead>
      <tbody>
        {list.map((c) => (
          <tr key={c.work}>
            <td>
              <Link to={to(c)}>{c.graph === 'architect' ? t('sys_architect_link') : `#${c.work}`}</Link>
            </td>
            <td>{c.item !== null ? <ItemRef id={c.item} title={c.title} /> : t('sys_architect_of', { project: c.project ?? '' })}</td>
            <td className="num">{c.n}</td>
            <td className="num">{cost(c.cost_usd)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// What went wrong in the period, by kind, each count opening its list.
function Anomalies({ s }: { s: SystemData }) {
  const groups = anomaliesBy(s.anomalies);
  return (
    <>
      <div className="us-tiles">
        {groups.map((g) => (
          <Tile
            key={g.kind}
            k={t(`sys_anomaly_${g.kind}`)}
            v={String(g.list.length)}
            open={`sys-anomaly-${g.kind}`}
            none={g.list.length === 0}
            hint={t(`sys_anomaly_${g.kind}_hint`)}
          />
        ))}
      </div>
      {groups
        .filter((g) => g.list.length > 0)
        .map((g) => (
          <Section key={g.kind} id={`sys-anomaly-${g.kind}`} title={t(`sys_anomaly_${g.kind}`)}>
            <AnomalyRows list={g.list} />
          </Section>
        ))}
    </>
  );
}

/// What went wrong, in words.
function anomalyWords(a: Anomaly): string {
  const w = a.what;
  switch (w.kind) {
    case 'flow_anomaly':
      return t('sys_what_flow_anomaly', { row: w.row.replaceAll('_', ' ') });
    case 'counter':
      return t('sys_what_counter', { counter: w.counter.replaceAll('_', ' '), limit: w.limit });
    case 'stopped':
      return w.why;
    case 'tool_failed':
      return t('sys_what_tool_failed', { cell: w.cell });
    case 'main_red': {
      const how =
        w.how === 'environment_changed'
          ? t('sys_main_environment')
          : w.how === 'broken_by_merge'
            ? t('sys_main_broken')
            : w.how === 'known_red'
              ? t('sys_main_known_red')
              : w.how;
      return w.checks.length > 0 ? t('sys_what_main_red', { how, checks: w.checks.join(', ') }) : how;
    }
  }
}

/// Anomalies, the latest first: when, the round it came of (the work, for the engine's stop), its item, and what went
/// wrong.
function AnomalyRows({ list }: { list: Anomaly[] }) {
  return (
    <table className="list us-table sys-table">
      <thead>
        <tr>
          <th>{t('sys_col_when')}</th>
          <th>{t('sys_col_where')}</th>
          <th>{t('sys_col_item')}</th>
          <th>{t('sys_col_what')}</th>
        </tr>
      </thead>
      <tbody>
        {list.map((a, i) => (
          <tr key={i}>
            <td>{time(a.at)}</td>
            <td>
              {a.graph === 'architect' ? (
                <Link
                  to={architectHref({ project: a.project, work: a.work, round: a.round })}
                  className={a.round !== null ? 'round-link' : undefined}
                >
                  {a.round !== null ? roundName(a.work, a.round) : t('sys_architect_link')}
                </Link>
              ) : a.round !== null ? (
                <RoundLink work={a.work} round={a.round} />
              ) : (
                <Link to={workHref(a.work)}>#{a.work}</Link>
              )}
            </td>
            <td>{a.item !== null ? <ItemRef id={a.item} title={a.title} /> : t('sys_architect_of', { project: a.project ?? '' })}</td>
            <td>{anomalyWords(a)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
