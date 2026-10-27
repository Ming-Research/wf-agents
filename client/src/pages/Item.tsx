// An item, what needs the owner first: the design cards and asks of the owner its live work waits on; where the work
// stands, as a checklist; the problem, as the engineer understands it once research has run; the design; the
// acceptance; what changed on the way; what it depends on; and, folded, its history: earlier and replaced design
// cards, answered asks, its works. Each block names the round it came from. What the works hand back is drawn by its
// shape in the flow's formats; a field the page does not know is shown as labelled text.

import { useEffect, type ReactNode } from 'react';

import type { DesignCard, ItemDetail, JsonValue, LinkedItem, OwnerAsk, Shown } from '../api';
import { Facts, ItemBadge, Loaded, Section, WorkBadge } from '../components';
import { cost, time } from '../format';
import { Acceptance, Approaches, Deviation, Design, Field, Fold, Measure, Understanding, type Met } from '../item/Shapes';
import { Progress } from '../item/Progress';
import { empty, isObj, label, Labelled, str } from '../labelled';
import { useResource } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { Link } from '../route';
import { t } from '../text';
import { AskView, bringIntoView, DesignCardView } from './Decide';

/// Where a value shown on the item came from: the work, the round and the cell, opening the round.
function From({ x }: { x: { work: number; round: number; cell?: string } }) {
  return (
    <Link to={`/works/${x.work}?round=${x.round}`} className="shown-from">
      {x.cell !== undefined
        ? t('shown_from', { work: x.work, round: x.round, cell: x.cell })
        : t('shown_from_round', { work: x.work, round: x.round })}
    </Link>
  );
}

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
  'end.followup',
  'end.conditions',
]);

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

/// A block of the page with the round it came from beside its title.
function Block(props: { title: ReactNode; from?: Shown; open?: boolean; children: ReactNode }) {
  return (
    <Section title={props.title} open={props.open} extra={props.from !== undefined ? <From x={props.from} /> : null}>
      {props.children}
    </Section>
  );
}

function Card({ c }: { c: DesignCard }) {
  return (
    <div id={`design-card-${c.id}`} className="boxed">
      <div className="box-head">
        <strong>{t('design_card_head', { id: c.id, status: c.status })}</strong> <From x={c} />
      </div>
      <DesignCardView c={c} />
    </div>
  );
}

function Ask({ a }: { a: OwnerAsk }) {
  return (
    <div id={`ask-${a.id}`} className="boxed">
      <div className="box-head">
        <strong>{t('ask_head', { id: a.id })}</strong> <From x={a} />
      </div>
      <AskView a={a} />
    </div>
  );
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
      <Fold title={t('item_as_written')}>{written}</Fold>
      {before.length > 0 ? (
        <Fold title={t('shown_before', { n: before.length })}>
          {before.map((x) => (
            <div key={x.round} className="shown-old">
              <From x={x} />
              <Understanding value={x.value} />
            </div>
          ))}
        </Fold>
      ) : null}
    </Block>
  );
}

/// One version of the design: why it was chosen, the design itself and the measure, as one round wrote them.
function DesignOf({ round, shown }: { round: number; shown: Map<string, Shown[]> }) {
  const at = (path: string): JsonValue | undefined => shown.get(path)?.find((x) => x.round === round)?.value;
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
  const rounds = [...new Set(marks.map((x) => x.round))].sort((a, b) => a - b);
  const options = newest(shown.get('options'));
  if (rounds.length === 0 && options === undefined) return null;
  const round = rounds[rounds.length - 1];
  const from = marks.find((x) => x.round === round);
  const chosen = str(shown.get('chosen')?.find((x) => x.round === round)?.value);
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
        <Fold
          title={
            <>
              {t('design_options', { n })} <From x={options} />
            </>
          }
        >
          <Approaches value={options.value} chosen={chosen} />
        </Fold>
      ) : null}
      {rounds.length > 1 ? (
        <Fold title={t('shown_before', { n: rounds.length - 1 })}>
          {rounds
            .slice(0, -1)
            .reverse()
            .map((r) => {
              const x = marks.find((y) => y.round === r);
              return (
                <div key={r} className="shown-old">
                  {x !== undefined ? <From x={x} /> : null}
                  <h3 className="design-chosen">
                    <MarkdownInline text={str(shown.get('chosen')?.find((y) => y.round === r)?.value)} />
                  </h3>
                  <DesignOf round={r} shown={shown} />
                </div>
              );
            })}
        </Fold>
      ) : null}
    </Block>
  );
}

/// The acceptance, once: the locked one with the round that locked it, or the one the design proposes before it is
/// locked; each condition with how it was met when the work has said.
function AcceptanceBlock({ d, shown }: { d: ItemDetail; shown: Map<string, Shown[]> }) {
  const proposed = newest(shown.get('acceptance'));
  const met = new Map<string, Met>();
  for (const x of shown.get('end.conditions') ?? []) {
    if (!Array.isArray(x.value)) continue;
    for (const m of x.value) if (isObj(m) && typeof m.id === 'string') met.set(m.id, m);
  }
  if (d.acceptance !== null) {
    const locked = d.acceptance_round !== null ? d.shown.find((x) => x.round === d.acceptance_round) : undefined;
    return (
      <Block title={t('acceptance')}>
        <p className="shown-from">
          {locked !== undefined ? (
            <Link to={`/works/${locked.work}?round=${locked.round}`} className="shown-from">
              {t('acceptance_locked', { round: locked.round })}
            </Link>
          ) : d.acceptance_round !== null ? (
            t('acceptance_locked', { round: d.acceptance_round })
          ) : null}
        </p>
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
  const followup = newest(shown.get('end.followup'));
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
          <Link to={`/items/${l.item.id}`}>
            #{l.item.id} {l.item.title}
          </Link>{' '}
          <ItemBadge state={l.item.state} />
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
  const imported = d.imported_from !== null || d.imported_branch !== null;
  if (cards.length === 0 && asks.length === 0 && d.works.length === 0 && !imported) return null;
  return (
    <Section title={t('history')} open={false}>
      {cards.length > 0 ? (
        <Fold title={t('history_cards', { n: cards.length })} open>
          {cards.map((c) => (
            <Card key={c.id} c={c} />
          ))}
        </Fold>
      ) : null}
      {asks.length > 0 ? (
        <Fold title={t('history_asks', { n: asks.length })} open>
          {asks.map((a) => (
            <Ask key={a.id} a={a} />
          ))}
        </Fold>
      ) : null}
      {d.works.length > 0 ? (
        <Fold title={t('history_works', { n: d.works.length })} open>
          <Works d={d} />
        </Fold>
      ) : null}
      {imported ? (
        <Fold title={t('item_imported')}>
          <p className="muted">{t('item_imported_note')}</p>
          <Facts
            rows={[
              [t('item_imported_from'), d.imported_from ?? ''],
              [t('branch'), d.imported_branch ?? ''],
              ['PR', d.imported_pr !== null ? `#${d.imported_pr}` : ''],
            ]}
          />
        </Fold>
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
          <span>
            {t('col_level')} {i.level}
          </span>
          <span>
            {t('col_deliver')} {i.deliver}
          </span>
          {i.pr !== null ? <span>PR #{i.pr}</span> : null}
        </div>
        {i.cancelled ? <div className="why">{i.cancelled}</div> : null}
      </header>
      {cards.length > 0 || asks.length > 0 ? (
        <Section title={t('needs_you')}>
          {asks.map((a) => (
            <Ask key={a.id} a={a} />
          ))}
          {cards.map((c) => (
            <Card key={c.id} c={c} />
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
            <Section title={t('depends_on')}>
              <Linked list={d.depends_on} />
            </Section>
          ) : null}
          {d.depended_on_by.length > 0 ? (
            <Section title={t('depended_on_by')}>
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
