// An item, what needs the owner first: the design cards and asks of the owner its live work waits on; where the work
// stands, as a checklist; the problem, as the engineer understands it once research has run; the design; the
// acceptance; what changed on the way; what it depends on; and, folded, its history: earlier and replaced design
// cards, answered asks, its works. Each block names the cell and round it came from. What the works hand back is
// drawn by its shape in the flow's formats; a field the page does not know is shown as labelled text.

import { useEffect, useState, type ReactNode } from 'react';

import type { DesignCard, ItemDetail, JsonValue, LinkedItem, OwnerAsk, Settings, Shown } from '../api';
import { DependencyKind, Facts, ItemBadge, Loaded, WorkBadge } from '../components';
import { cost, time } from '../format';
import { Acceptance, Approaches, Deviation, Design, Field, Measure, Understanding, type Met } from '../item/Shapes';
import { Progress } from '../item/Progress';
import { empty, isObj, label, Labelled, str } from '../labelled';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { By, ItemRef, RoundLink, Section } from '../parts';
import { Link } from '../route';
import { t } from '../text';
import { AskView, bringIntoView, currentCard, DesignCardView } from './Decide';

/// The tiers an item may be put at: those up to the highest an item may be at, in order.
export function tiersAllowed(s: Settings): string[] {
  const names = s.tiers.map((x) => x.name);
  const top = s.highest_level === null ? -1 : names.indexOf(s.highest_level);
  return top < 0 ? names : names.slice(0, top + 1);
}

/// The item's tier, which the owner changes here: a tier above the highest an item may be at is not offered. Its works
/// run at the new tier from their next round.
function Tier({ id, level }: { id: number; level: string }) {
  const s = useResource<Settings>('settings', '/api/v1/settings');
  const [error, setError] = useState<string | null>(null);
  const allowed = s.data !== undefined ? tiersAllowed(s.data) : [];
  const options = allowed.includes(level) ? allowed : [...allowed, level];
  return (
    <span>
      {t('col_level')}{' '}
      <select
        value={level}
        onChange={(e) => {
          setError(null);
          postJson(`/api/v1/items/${id}/level`, { level: e.target.value }).then(
            () => reread(`item:${id}`, 'items'),
            (x: unknown) => setError(x instanceof Error ? x.message : String(x)),
          );
        }}
      >
        {options.map((n) => (
          <option key={n} value={n} disabled={!allowed.includes(n)}>
            {n}
          </option>
        ))}
      </select>
      {error !== null ? <span className="bad"> {error}</span> : null}
    </span>
  );
}

/// Where a value shown on the item came from: the cell and the round, opening the round.
function From({ x }: { x: { work: number; round: number; cell?: string } }) {
  return <By cell={x.cell} work={x.work} round={x.round} />;
}

/// A round of one of the item's works: two values shown come from the same round when this is the same.
const roundKey = (x: { work: number; round: number }) => `${x.work}-${x.round}`;

/// The paths of a return the page draws in a place of its own; any other path the flow shows on the item is listed
/// as labelled text. Messages to the owner are on the messages view; the design tree's change is the design card.
const DRAWN = new Set([
  'progress',
  'tell_owner',
  'understanding',
  'options',
  'chosen',
  'why',
  'measure',
  'design',
  'design_tree',
  'acceptance',
  'deviations',
  'followup',
  'conditions',
  'end.followup',
  'end.conditions',
]);

/// The values shown under a path of the answer of implementing and fixing, oldest first: those of a flow version
/// before that answer was flat, which held them under end, come first.
function done(shown: Map<string, Shown[]>, path: string): Shown[] {
  return [...(shown.get(`end.${path}`) ?? []), ...(shown.get(path) ?? [])];
}

/// The values shown on the item, by path, oldest first, those with nothing left out.
function byPath(d: ItemDetail): Map<string, Shown[]> {
  const out = new Map<string, Shown[]>();
  for (const x of d.shown) {
    if (empty(x.value)) continue;
    out.set(x.path, [...(out.get(x.path) ?? []), x]);
  }
  return out;
}

const newest = (list: Shown[] | undefined): Shown | undefined =>
  list !== undefined && list.length > 0 ? list[list.length - 1] : undefined;

/// A block of the page, open unless said, with the cell and round it came from beside its title.
function Block(props: { title: ReactNode; from?: Shown; open?: boolean; children: ReactNode }) {
  return (
    <Section title={props.title} open={props.open ?? true} by={props.from !== undefined ? <From x={props.from} /> : undefined}>
      {props.children}
    </Section>
  );
}

/// A design card, in the card every card shares, linked to the round that handed it in; folded to its one line unless
/// it is the item's current card.
function Card({ c, current }: { c: DesignCard; current: number | null }) {
  return <DesignCardView c={c} round={<RoundLink work={c.work} round={c.round} />} folded={c.id !== current} />;
}

function Ask({ a }: { a: OwnerAsk }) {
  return <AskView a={a} round={<RoundLink work={a.work} round={a.round} />} />;
}

/// The problem: the engineer's understanding once research has run, the item as its creator wrote it folded under it;
/// before that, the item as written.
function Problem({ d, shown }: { d: ItemDetail; shown: Map<string, Shown[]> }) {
  const all = shown.get('understanding') ?? [];
  const now = newest(all);
  const written = (
    <>
      <Markdown text={d.problem} />
      <Field label={t('why')} value={d.why} />
      <Field label={t('approach')} value={d.approach} />
    </>
  );
  if (now === undefined) return <Block title={t('problem_title')}>{written}</Block>;
  const before = all.slice(0, -1).reverse();
  return (
    <Block title={t('problem_title')} from={now}>
      <Understanding value={now.value} />
      <Section title={t('item_as_written')}>{written}</Section>
      <Section title={t('shown_before')} n={before.length}>
        {before.map((x) => (
          <div key={roundKey(x)} className="shown-old">
            <From x={x} />
            <Understanding value={x.value} />
          </div>
        ))}
      </Section>
    </Block>
  );
}

/// One version of the design: why it was chosen, the design itself and the measure, as one round wrote them.
function DesignOf({ round, shown }: { round: string; shown: Map<string, Shown[]> }) {
  const at = (path: string): JsonValue | undefined => shown.get(path)?.find((x) => roundKey(x) === round)?.value;
  const why = at('why');
  const design = at('design');
  const measure = at('measure');
  const measured = measure !== undefined ? <Measure value={measure} /> : null;
  return (
    <>
      {why !== undefined ? <Field label={t('design_why')} value={why} /> : null}
      {design !== undefined ? <Design value={design} after={measured} /> : measured}
    </>
  );
}

/// The design: the approach chosen as its heading, why, the design itself and the measure; the approaches compared
/// folded; earlier versions folded.
function DesignBlock({ shown }: { shown: Map<string, Shown[]> }) {
  const marks = ['chosen', 'why', 'design', 'measure'].flatMap((p) => shown.get(p) ?? []);
  // The rounds that wrote a version, oldest first: by their works, then their numbers in the work.
  const rounds = [...new Map(marks.map((x) => [roundKey(x), x])).values()]
    .sort((a, b) => a.work - b.work || a.round - b.round)
    .map(roundKey);
  const options = newest(shown.get('options'));
  if (rounds.length === 0 && options === undefined) return null;
  const round = rounds[rounds.length - 1];
  const from = marks.find((x) => roundKey(x) === round);
  const chosen = str(shown.get('chosen')?.find((x) => roundKey(x) === round)?.value);
  const n = options !== undefined && Array.isArray(options.value) ? options.value.length : 0;
  return (
    <Block title={t('design_title')} from={from ?? options}>
      {chosen !== '' ? (
        <h3 className="design-chosen">
          <MarkdownInline text={chosen} />
        </h3>
      ) : null}
      {round !== undefined ? <DesignOf round={round} shown={shown} /> : null}
      {options !== undefined ? (
        <Section title={t('design_options')} n={n} by={<From x={options} />}>
          <Approaches value={options.value} chosen={chosen} />
        </Section>
      ) : null}
      {rounds.length > 1 ? (
        <Section title={t('shown_before')} n={rounds.length - 1}>
          {rounds
            .slice(0, -1)
            .reverse()
            .map((r) => {
              const x = marks.find((y) => roundKey(y) === r);
              return (
                <div key={r} className="shown-old">
                  {x !== undefined ? <From x={x} /> : null}
                  <h3 className="design-chosen">
                    <MarkdownInline text={str(shown.get('chosen')?.find((y) => roundKey(y) === r)?.value)} />
                  </h3>
                  <DesignOf round={r} shown={shown} />
                </div>
              );
            })}
        </Section>
      ) : null}
    </Block>
  );
}

/// The acceptance, once: the locked one with the round that locked it, or the one the design proposes before it is
/// locked; each condition with how it was met when the work has said.
function AcceptanceBlock({ d, shown }: { d: ItemDetail; shown: Map<string, Shown[]> }) {
  const proposed = newest(shown.get('acceptance'));
  const met = new Map<string, Met>();
  for (const x of done(shown, 'conditions')) {
    if (!Array.isArray(x.value)) continue;
    for (const m of x.value) if (isObj(m) && typeof m.id === 'string') met.set(m.id, m);
  }
  if (d.acceptance !== null) {
    const at = d.acceptance_work !== null && d.acceptance_round !== null
      ? { work: d.acceptance_work, round: d.acceptance_round }
      : null;
    return (
      <Block title={t('acceptance')}>
        {at !== null ? (
          <p>
            <By work={at.work} round={at.round}>
              {t('acceptance_locked')}
            </By>
          </p>
        ) : null}
        <Acceptance value={d.acceptance} met={met} />
      </Block>
    );
  }
  if (proposed === undefined) return null;
  return (
    <Block title={t('acceptance')} from={proposed}>
      <p className="muted">{t('acceptance_proposed')}</p>
      <Acceptance value={proposed.value} met={met} />
    </Block>
  );
}

/// What changed on the way: where the work departed from the design, newest first, and what to do next.
function Changes({ shown }: { shown: Map<string, Shown[]> }) {
  const deviations = (shown.get('deviations') ?? [])
    .flatMap((x) => (Array.isArray(x.value) ? x.value.map((v) => ({ x, v })) : [{ x, v: x.value }]))
    .reverse();
  const followup = newest(done(shown, 'followup'));
  if (deviations.length === 0 && followup === undefined) return null;
  return (
    <Block title={t('changes_title')}>
      {deviations.map(({ x, v }, i) => (
        <div key={i} className="change">
          <Deviation value={v} /> <From x={x} />
        </div>
      ))}
      {followup !== undefined ? (
        <div className="change">
          <span className="decision-label">{t('change_followup')}</span> <From x={followup} />
          <Labelled value={followup.value} />
        </div>
      ) : null}
    </Block>
  );
}

/// Anything else the flow shows on the item that the page does not draw in a place of its own, at its newest.
function Others({ shown }: { shown: Map<string, Shown[]> }) {
  const paths = [...shown.keys()].filter((p) => !DRAWN.has(p));
  if (paths.length === 0) return null;
  return (
    <Block title={t('other_found')}>
      {paths.map((p) => {
        const x = newest(shown.get(p));
        return x === undefined ? null : (
          <div key={p} className="field">
            <span className="decision-label">{label(p)}</span> <From x={x} />
            <Labelled value={x.value} />
          </div>
        );
      })}
    </Block>
  );
}

function Linked({ list }: { list: LinkedItem[] }) {
  return (
    <ul className="linked">
      {list.map((l) => (
        <li key={l.item.id}>
          <ItemRef id={l.item.id} title={l.item.title} />{' '}
          <ItemBadge state={l.item.state} /> <DependencyKind kind={l.kind} />
          <div className="muted">{l.needs}</div>
        </li>
      ))}
    </ul>
  );
}

function Works({ d }: { d: ItemDetail }) {
  return (
    <table className="list">
      <tbody>
        {d.works.map((w) => (
          <tr key={w.id}>
            <td>
              <Link to={`/works/${w.id}`}>#{w.id}</Link>
            </td>
            <td>
              <WorkBadge state={w.state} />
            </td>
            <td>
              <span className="cell-id">{w.at}</span> {w.at_title}
            </td>
            <td>{time(w.started_at)}</td>
            <td className="num">{w.rounds}</td>
            <td className="num">{cost(w.cost_usd)}</td>
            <td>{w.pr !== null ? `PR #${w.pr}` : ''}</td>
            <td className="muted">{w.ended ?? ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// What the item went through, folded: earlier and replaced design cards, earlier asks, its works, what an import
/// recorded; nothing when it has none of these.
function History({ d, cards, asks }: { d: ItemDetail; cards: DesignCard[]; asks: OwnerAsk[] }) {
  const current = currentCard(d.design_cards);
  const imported = d.imported_from !== null || d.imported_branch !== null;
  if (cards.length === 0 && asks.length === 0 && d.works.length === 0 && !imported) return null;
  return (
    <Section title={t('history')} open={false}>
      <Section title={t('history_cards')} n={cards.length} open>
        <p className="dc-lead">{t('dc_lead')}</p>
        {cards.map((c) => (
          <Card key={c.id} c={c} current={current} />
        ))}
      </Section>
      <Section title={t('history_asks')} n={asks.length} open>
        {asks.map((a) => (
          <Ask key={a.id} a={a} />
        ))}
      </Section>
      <Section title={t('history_works')} n={d.works.length} open>
        <Works d={d} />
      </Section>
      {imported ? (
        <Section title={t('item_imported')}>
          <p className="muted">{t('item_imported_note')}</p>
          <Facts
            rows={[
              [t('item_imported_from'), d.imported_from ?? ''],
              [t('branch'), d.imported_branch ?? ''],
              ['PR', d.imported_pr !== null ? `#${d.imported_pr}` : ''],
            ]}
          />
        </Section>
      ) : null}
    </Section>
  );
}

function Detail({ d }: { d: ItemDetail }) {
  const i = d.item;
  const shown = byPath(d);
  // What needs the owner: the design cards and asks a work that still goes on waits on.
  const live = new Set(d.works.filter((w) => w.state !== 'done' && w.state !== 'ended').map((w) => w.id));
  const cards = d.design_cards.filter((c) => c.status === 'open' && live.has(c.work));
  const asks = d.asks.filter((a) => a.answer === null && live.has(a.work));
  const earlierCards = d.design_cards.filter((c) => !cards.includes(c)).reverse();
  const earlierAsks = d.asks.filter((a) => !asks.includes(a)).reverse();
  // A link to a design card or an ask on this page brings it into view once the page is there.
  useEffect(() => {
    const at = window.location.hash.slice(1);
    if (at !== '') bringIntoView(at);
  }, [i.id]);
  return (
    <>
      <header className="work-head">
        <h1>
          {t('item_title', { id: i.id })} <span className="head-item">{i.title}</span>
          <ItemBadge state={i.state} />
        </h1>
        <div className="head-facts">
          <span>{i.project}</span>
          <Tier id={i.id} level={i.level} />
          <span>
            {t('col_deliver')} {i.deliver}
          </span>
          {i.pr !== null ? <span>PR #{i.pr}</span> : null}
        </div>
        {i.cancelled ? <div className="why">{i.cancelled}</div> : null}
        {i.blocked_by.length > 0 ? (
          <div className="muted">
            {t('item_blocked', { items: i.blocked_by.map((n) => `#${n}`).join(', ') })}
          </div>
        ) : null}
        {d.level_changes.map((c, k) => (
          <div key={k} className="muted">
            {t('tier_changed', { from: c.from, to: c.to, by: c.by, at: time(c.at) })}
          </div>
        ))}
      </header>
      {cards.length > 0 || asks.length > 0 ? (
        <Section title={t('needs_you')} open>
          {asks.map((a) => (
            <Ask key={a.id} a={a} />
          ))}
          {cards.map((c) => (
            <Card key={c.id} c={c} current={c.id} />
          ))}
        </Section>
      ) : null}
      <Block title={t('progress')}>
        <Progress d={d} />
      </Block>
      <Problem d={d} shown={shown} />
      <DesignBlock shown={shown} />
      <AcceptanceBlock d={d} shown={shown} />
      <Changes shown={shown} />
      <Others shown={shown} />
      {d.depends_on.length > 0 || d.depended_on_by.length > 0 ? (
        <div className="two">
          {d.depends_on.length > 0 ? (
            <Section title={t('depends_on')} open>
              <Linked list={d.depends_on} />
            </Section>
          ) : null}
          {d.depended_on_by.length > 0 ? (
            <Section title={t('depended_on_by')} open>
              <Linked list={d.depended_on_by} />
            </Section>
          ) : null}
        </div>
      ) : null}
      <History d={d} cards={earlierCards} asks={earlierAsks} />
    </>
  );
}

/// An item's page, on the items view beside the tree.
export function Item({ id }: { id: number }) {
  const item = useResource<ItemDetail>(`item:${id}`, `/api/v1/items/${id}`);
  return (
    <div className="page item-page">
      <Loaded r={item}>{(d) => <Detail d={d} />}</Loaded>
    </div>
  );
}
