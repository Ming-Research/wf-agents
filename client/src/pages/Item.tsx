// An item, read only: its number and title with its facts on the title's line, and the work that goes on under them;
// then what waits for the owner, the decision cards and asks of the owner its live work waits on, each saying where it
// is ruled on or answered, its work's group in the messages view; where its live work stands as a checklist; the
// problem, as the engineer understands it once research has run; the design, saying so while no approach is chosen;
// the acceptance; what changed on the way; what it depends on and what depends on it, each dependency one the owner
// can remove; and, folded, its history: earlier and replaced decision cards, answered asks, and the works that ended.
// Each block names the cell and round it came from. What the works hand back is drawn by its shape in the flow's
// formats; a field the page does not know is shown as labelled text.

import { XIcon } from '@primer/octicons-react';
import { ConfirmationDialog, Flash, IconButton, PageHeader } from '@primer/react';
import { Fragment, useEffect, useState, type ReactNode } from 'react';

import type { DesignCard, ItemDetail, ItemSummary, JsonValue, LinkedItem, OwnerAsk, Shown, Waits } from '../api';
import { course, DependencyKind, Facts, ItemBadge, liveState, Loaded, StateChip, WorkBadge } from '../components';
import { cost, time } from '../format';
import {
  Acceptance,
  anchored,
  Approaches,
  CardsAtAnchors,
  Deviation,
  Design,
  Field,
  Measure,
  Understanding,
  type Met,
} from '../item/Shapes';
import { Progress } from '../item/Progress';
import { empty, isObj, label, Labelled, str, words } from '../labelled';
import { waitsWords } from '../course';
import { postJson } from '../http';
import { reread, useResource, useStatus } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { By, HeadingLevel, ItemRef, PrLink, RoundLink, Section, SubHeading } from '../parts';
import { href, Link } from '../route';
import { t, tn } from '../text';
import { factsAfter, joined } from '../voices';
import { AskView, bringIntoView, cardOfKey, currentCard, DesignCardView } from './Decide';

/// Where a value shown on the item came from: the cell and the round, opening the round.
function From({ x }: { x: { work: number; round: number; cell?: string } }) {
  return <By cell={x.cell} work={x.work} round={x.round} />;
}

/// A round of one of the item's works: two values shown come from the same round when this is the same.
const roundKey = (x: { work: number; round: number }) => `${x.work}-${x.round}`;

/// The paths of a return the page draws in a place of its own; any other path the flow shows on the item is listed
/// as labelled text. Messages to the owner are on the messages view; the design tree's change is the decision card.
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
  'design_updates',
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
  return pathsOf(d.shown);
}

/// Values shown, by path, oldest first, those with nothing left out.
function pathsOf(list: Shown[]): Map<string, Shown[]> {
  const out = new Map<string, Shown[]>();
  for (const x of list) {
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

/// A decision card, in the card every card shares, linked to the round that handed it in; folded to its one line unless
/// it is the item's current card.
function Card({ c, current }: { c: DesignCard; current: number | null }) {
  return <DesignCardView c={c} round={<RoundLink work={c.work} round={c.round} />} folded={c.id !== current} />;
}

/// The decision cards that wait for the owner in the item's design, in one line, each number bringing its card into
/// view.
function InDesign({ cards }: { cards: DesignCard[] }) {
  const links = cards.map((c) => (
    <a
      key={c.id}
      href={`#design-card-${c.id}`}
      onClick={(e) => {
        e.preventDefault();
        bringIntoView(`design-card-${c.id}`);
      }}
    >
      {t('card_name', { id: c.id })}
    </a>
  ));
  return (
    <p className="needs-in-design">
      {t('needs_in_design')} {links.flatMap((l, i) => (i === 0 ? [l] : [', ', l]))}
    </p>
  );
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
  // The design itself first, then, folded, why this one and not the others: the owner reads a design to see what it
  // is.
  return (
    <>
      {design !== undefined ? <Design value={design} after={measured} /> : measured}
      {why !== undefined ? (
        <Section title={t('design_why')} open={false}>
          {typeof why === 'string' ? <Markdown text={why} /> : <Labelled value={why} />}
        </Section>
      ) : null}
    </>
  );
}

/// Whether a value shown came after another: a later work, or a later round of the same work.
const after = (a: Shown, b: Shown): boolean => a.work > b.work || (a.work === b.work && a.round > b.round);

/// While the approaches compared wait for one to be chosen: so, plainly, where the engineer stands on them as its
/// latest round said it (`leans`), and what the choice waits for (`waits`), as its work says it.
function NoChoice({ n, leans, waits }: { n: number; leans: string | null; waits: Waits | null }) {
  return (
    <Flash variant="warning" className="design-none">
      <p>{tn('design_none_chosen', n)}</p>
      {leans !== null ? (
        <>
          <p>{t('design_leans')}</p>
          <div className="agent-box">
            <Markdown text={leans} />
          </div>
        </>
      ) : null}
      {waits !== null ? <p>{t('design_waits', { what: waitsWords(waits) })}</p> : null}
    </Flash>
  );
}

/// The design as the rounds that wrote it handed it back: the approach chosen as its heading, the design itself and the
/// measure, then why this one, folded; the approaches compared folded, the one chosen marked by the choice that came
/// after them, and while none came, so, with where the engineer stands (`leans`) and what the choice waits for
/// (`waits`); earlier versions folded. In a stage card of a work's group as on the item.
export function DesignShown({ shown, leans, waits }: { shown: Shown[]; leans: string | null; waits: Waits | null }) {
  return <DesignParts shown={pathsOf(shown)} leans={leans} waits={waits} acceptance />;
}

function DesignParts({ shown, leans, waits, acceptance = false }: {
  shown: Map<string, Shown[]>;
  leans: string | null;
  waits: Waits | null;
  acceptance?: boolean;
}) {
  const marks = ['chosen', 'why', 'design', 'measure'].flatMap((p) => shown.get(p) ?? []);
  // The rounds that wrote a version, oldest first: by their works, then their numbers in the work.
  const rounds = [...new Map(marks.map((x) => [roundKey(x), x])).values()]
    .sort((a, b) => a.work - b.work || a.round - b.round)
    .map(roundKey);
  const options = newest(shown.get('options'));
  const round = rounds[rounds.length - 1];
  const choice = shown.get('chosen')?.find((x) => roundKey(x) === round);
  const chosen = str(choice?.value);
  // The approaches compared are chosen among only by a choice made after them.
  const chose = options !== undefined && choice !== undefined && after(choice, options) ? chosen : '';
  const n = options !== undefined && Array.isArray(options.value) ? options.value.length : 0;
  const proposed = acceptance ? newest(shown.get('acceptance')) : undefined;
  return (
    <>
      {options !== undefined && chose === '' ? <NoChoice n={n} leans={leans} waits={waits} /> : null}
      {chosen !== '' && (options === undefined || chose !== '') ? (
        <SubHeading className="design-chosen">
          <MarkdownInline text={chosen} />
        </SubHeading>
      ) : null}
      {round !== undefined ? <DesignOf round={round} shown={shown} /> : null}
      {options !== undefined ? (
        <Section title={t('design_options')} n={n} by={<From x={options} />}>
          <Approaches value={options.value} chosen={chose} />
        </Section>
      ) : null}
      {proposed !== undefined ? (
        <Section title={t('acceptance')} by={<From x={proposed} />}>
          <Acceptance value={proposed.value} met={new Map()} />
        </Section>
      ) : null}
      {rounds.length > 1 ? (
        <CardsAtAnchors.Provider value={null}>
        <Section title={t('shown_before')} n={rounds.length - 1}>
          {rounds
            .slice(0, -1)
            .reverse()
            .map((r) => {
              const x = marks.find((y) => roundKey(y) === r);
              return (
                <div key={r} className="shown-old">
                  {x !== undefined ? <From x={x} /> : null}
                  <SubHeading className="design-chosen">
                    <MarkdownInline text={str(shown.get('chosen')?.find((y) => roundKey(y) === r)?.value)} />
                  </SubHeading>
                  <DesignOf round={r} shown={shown} />
                </div>
              );
            })}
        </Section>
        </CardsAtAnchors.Provider>
      ) : null}
    </>
  );
}

/// The decision cards the item's design stands, by their anchors in its newest text: the card of each key among those
/// of the work that wrote it.
function cardsInDesign(d: ItemDetail, shown: Map<string, Shown[]>): DesignCard[] {
  const design = newest(shown.get('design'));
  if (design === undefined || !isObj(design.value) || typeof design.value.solution !== 'string') return [];
  return anchored(design.value.solution).flatMap((p) => {
    const c = 'key' in p ? cardOfKey(d.design_cards, design.work, p.key) : null;
    return c !== null ? [c] : [];
  });
}

/// A passage of the design as implementing or fixing updated it, with the round it first came from.
export type Update = { passage: string; decision: string; why: string; from: Shown };

/// The design as updated while implementing and fixing, oldest first: of the latest work that updated it, each cell's
/// newest return holds every passage that cell has written, so its rows are those that stand; each is shown with the
/// earliest round of that cell whose return held it word for word, so that a passage handed in again by later rounds
/// is shown once, where it came from.
export function designUpdates(list: Shown[] | undefined): Update[] {
  const all = (list ?? []).filter((x) => Array.isArray(x.value));
  if (all.length === 0) return [];
  const work = Math.max(...all.map((x) => x.work));
  const ofWork = all.filter((x) => x.work === work).sort((a, b) => a.round - b.round);
  const same = (a: JsonValue, b: JsonValue) => JSON.stringify(a) === JSON.stringify(b);
  const out: Update[] = [];
  for (const cell of [...new Set(ofWork.map((x) => x.cell))]) {
    const rounds = ofWork.filter((x) => x.cell === cell);
    const now = rounds[rounds.length - 1];
    for (const row of now.value as JsonValue[]) {
      if (!isObj(row)) continue;
      const first = rounds.find((x) => (x.value as JsonValue[]).some((r) => same(r, row))) ?? now;
      out.push({ passage: str(row.passage), decision: str(row.decision), why: str(row.why), from: first });
    }
  }
  return out.sort((a, b) => a.from.round - b.from.round);
}

/// The design as updated while implementing and fixing, after the design: each passage with its decision and why, and
/// the round it came from.
export function DesignUpdates({ updates }: { updates: Update[] }) {
  if (updates.length === 0) return null;
  return (
    <Section title={t('design_updated')} n={updates.length} open>
      {updates.map((u, i) => (
        <div key={i} className="change">
          <div className="change-head">
            <SubHeading>
              <MarkdownInline text={u.passage} />
            </SubHeading>
            <From x={u.from} />
          </div>
          <Markdown text={u.decision} />
          <Field label={t('why')} value={u.why} />
        </div>
      ))}
    </Section>
  );
}

/// The design on the item: under its heading, as the rounds that wrote it handed it back, each decision card in it at
/// its anchor, read only, saying where it is ruled; while no approach is chosen, where the engineer of its latest work
/// stands and what the choice waits for; then the design as updated while implementing and fixing, which a work that
/// came straight from its research has alone.
function DesignBlock({ shown, d }: { shown: Map<string, Shown[]>; d: ItemDetail }) {
  const marks = ['chosen', 'why', 'design', 'measure', 'options'].flatMap((p) => shown.get(p) ?? []);
  // From every value shown, an empty list included: a cell's newest return that holds none takes its passages back.
  const updates = designUpdates(d.shown.filter((x) => x.path === 'design_updates'));
  const all = [...marks, ...updates.map((u) => u.from)];
  if (all.length === 0) return null;
  const from = all.reduce((a, b) => (after(b, a) ? b : a));
  const live = d.works.find((w) => w.state !== 'done' && w.state !== 'ended') ?? null;
  const work = newest(shown.get('design'))?.work ?? from.work;
  const at = (key: string): ReactNode => {
    const c = cardOfKey(d.design_cards, work, key);
    return c !== null ? <Card c={c} current={c.id} /> : null;
  };
  return (
    <Block title={t('design_title')} from={from}>
      <div className="document">
        <CardsAtAnchors.Provider value={at}>
          <DesignParts shown={shown} leans={null} waits={live?.waits ?? null} />
        </CardsAtAnchors.Provider>
        <DesignUpdates updates={updates} />
      </div>
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
          <div className="change-head">
            <SubHeading>{t('change_followup')}</SubHeading>
            <From x={followup} />
          </div>
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
            <div className="change-head">
              <SubHeading>{label(p)}</SubHeading>
              <From x={x} />
            </div>
            <Labelled value={x.value} />
          </div>
        );
      })}
    </Block>
  );
}

function Linked({ list, pair }: { list: LinkedItem[]; pair: (other: number) => [number, number] }) {
  return (
    <ul className="linked">
      {list.map((l) => (
        <li key={l.item.id}>
          <div className="linked-head">
            <ItemRef id={l.item.id} title={l.item.title} />
            <ItemBadge state={l.item.state} />
            <DependencyKind kind={l.kind} />
            <Remove link={pair(l.item.id)} />
          </div>
          <div className="muted">{l.needs}</div>
        </li>
      ))}
    </ul>
  );
}

/// The owner's removal of a dependency, a small IconButton at the end of its row saying on hovering what it does,
/// which asks first in Primer's confirmation dialog, since a work that waits for the other item goes on without it,
/// and the page has no way to make the dependency again; the request is sent only once the owner confirms.
function Remove({ link }: { link: [number, number] }) {
  const [item, on] = link;
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const remove = () => {
    setBusy(true);
    setError(null);
    postJson(`/api/v1/items/${item}/depends-on/${on}/remove`, {}).then(
      () => {
        setBusy(false);
        reread('items', 'item:*', 'works', 'work:*');
      },
      (x: unknown) => {
        setBusy(false);
        setError(x instanceof Error ? x.message : String(x));
      },
    );
  };
  const name = t('dependency_remove');
  return (
    <>
      <IconButton
        size="small"
        variant="invisible"
        icon={XIcon}
        onClick={() => setAsking(true)}
        disabled={busy}
        title={`${name}\n${t('dependency_remove_hint', { item, on })}`}
        aria-label={name}
        unsafeDisableTooltip
      />
      {error !== null ? <span className="linked-error">{error}</span> : null}
      {asking ? (
        <ConfirmationDialog
          title={t('dependency_remove_title', { item, on })}
          confirmButtonContent={name}
          confirmButtonType="danger"
          onClose={(gesture) => {
            setAsking(false);
            if (gesture === 'confirm') remove();
          }}
        >
          {t('dependency_remove_body', { item, on })}
        </ConfirmationDialog>
      ) : null}
    </>
  );
}

function Works({ works }: { works: ItemDetail['works'] }) {
  const status = useStatus();
  return (
    <table className="list">
      <tbody>
        {works.map((w) => (
          <tr key={w.id}>
            <td>
              <Link to={`/works/${w.id}`}>{t('work_name', { id: w.id })}</Link>
            </td>
            <td>
              <WorkBadge state={liveState(w, status)} />
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

/// What the item went through, folded: earlier and replaced decision cards, earlier asks, the works that ended, what an
/// import recorded; nothing when it has none of these. The work that goes on is at the top of the page, once.
function History({ d, cards, asks }: { d: ItemDetail; cards: DesignCard[]; asks: OwnerAsk[] }) {
  const current = currentCard(d.design_cards);
  const imported = d.imported_from !== null || d.imported_branch !== null;
  const ended = d.works.filter((w) => w.state === 'done' || w.state === 'ended');
  if (cards.length === 0 && asks.length === 0 && ended.length === 0 && !imported) return null;
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
      <Section title={t('history_works')} n={ended.length} open>
        <Works works={ended} />
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

/// The way the item's latest work went, as a fact of its head, when it is not the way most go (through the design): on
/// the research's plan straight away, or begun so and then sent to the design, what each leaves out or adds said on
/// hovering. Nothing before the research answers, and nothing for a work that went through the design.
export function routeFact(i: Pick<ItemSummary, 'route_taken'>): ReactNode {
  if (i.route_taken === 'direct') {
    return <span title={t('item_route_direct_hint')}>{t('item_route_direct')}</span>;
  }
  if (i.route_taken === 'turned') {
    return <span title={t('item_route_turned_hint')}>{t('item_route_turned')}</span>;
  }
  return null;
}

/// The item's head: its number and title, and its facts in two lines, each kind in its one look, the facts muted words,
/// the links in the accent, joined by a bar. On the title's line after a bar, smaller, wrapping under it where the line
/// is short: its project, its tier (how it changed said on hovering; a goal has none), the way its work went when that is not through
/// the design (`routeFact`), its state as its chip with the items it waits for after it, and its pull request. Under it, while a work goes on on it: that work, once, by its number with its
/// state's chip, its stage, its cell, what it waits for, and its group, where the owner speaks to it and its tier is
/// set.
function Head({ d }: { d: ItemDetail }) {
  const status = useStatus();
  const i = d.item;
  const w = d.works.find((x) => x.state !== 'done' && x.state !== 'ended');
  const changes = d.level_changes.map((c) => t('tier_changed', { from: c.from, to: c.to, by: c.by, at: time(c.at) }));
  const facts = factsAfter([
    i.project,
    // A goal has no work, and so no tier.
    i.level !== null ? (
      <span title={changes.length > 0 ? changes.join('\n') : undefined}>{t('item_tier', { tier: i.level })}</span>
    ) : null,
    routeFact(i),
    <span className="item-head-pair">
      <ItemBadge state={i.state} />
      {i.blocked_by.length > 0 ? (
        <StateChip tone="attention" title={t('items_blocked_title')}>
          {t('items_blocked', { items: i.blocked_by.map((n) => `#${n}`).join(', ') })}
        </StateChip>
      ) : null}
    </span>,
    i.pr !== null ? <PrLink project={i.project} n={i.pr} /> : null,
  ]);
  // What the work waits for as its course says it, which a work held back by every work's pause does not wait to retry.
  const waiting = w === undefined ? null : course(w, status).waits;
  const live =
    w === undefined
      ? []
      : joined([
          <span className="item-head-pair">
            <Link to={`/works/${w.id}`}>{t('item_live_work', { id: w.id })}</Link>
            <WorkBadge state={liveState(w, status)} />
          </span>,
          w.stage !== null ? words(w.stage) : null,
          <>
            <span className="cell-id">{w.at}</span> {w.at_title}
          </>,
          waiting !== null ? waitsWords(waiting) : null,
          <Link to={href({ page: 'messages', open: `work:${w.id}` })} title={t('go_to_conversation_hint')}>
            {t('go_to_conversation')}
          </Link>,
        ]);
  return (
    <PageHeader as="header" className="item-head">
      <PageHeader.TitleArea className="item-head-title">
        <PageHeader.Title as="h1">
          <span className="item-head-id">#{i.id}</span> {i.title}
        </PageHeader.Title>
        <span className="item-head-facts muted">
          {facts.map((x, k) => (
            <Fragment key={k}>{x}</Fragment>
          ))}
        </span>
      </PageHeader.TitleArea>
      {live.length > 0 ? (
        <PageHeader.Description className="item-head-facts muted">
          {live.map((x, k) => (
            <Fragment key={k}>{x}</Fragment>
          ))}
        </PageHeader.Description>
      ) : null}
    </PageHeader>
  );
}

function Detail({ d }: { d: ItemDetail }) {
  const i = d.item;
  const shown = byPath(d);
  // What needs the owner: the decision cards and asks a work that still goes on waits on. A card that stands in the
  // design is drawn there, and named here in one line that leads to it; a design card of before is drawn here.
  const live = new Set(d.works.filter((w) => w.state !== 'done' && w.state !== 'ended').map((w) => w.id));
  const inDesign = cardsInDesign(d, shown);
  const waiting = d.design_cards.filter((c) => c.status === 'open' && live.has(c.work));
  const cards = waiting.filter((c) => !inDesign.includes(c));
  const there = waiting.filter((c) => inDesign.includes(c));
  const asks = d.asks.filter((a) => a.answer === null && live.has(a.work));
  const earlierCards = d.design_cards.filter((c) => !cards.includes(c) && !inDesign.includes(c)).reverse();
  const earlierAsks = d.asks.filter((a) => !asks.includes(a)).reverse();
  // A link to a decision card or an ask on this page brings it into view once the page is there.
  useEffect(() => {
    const at = window.location.hash.slice(1);
    if (at !== '') bringIntoView(at);
  }, [i.id]);
  return (
    <HeadingLevel.Provider value={2}>
      <Head d={d} />
      {i.cancelled ? (
        <Flash variant="warning" className="item-cancelled">
          {i.cancelled}
        </Flash>
      ) : null}
      {cards.length > 0 || asks.length > 0 || there.length > 0 ? (
        <Section title={t('needs_you')} open>
          {there.length > 0 ? <InDesign cards={there} /> : null}
          {asks.map((a) => (
            <Ask key={a.id} a={a} />
          ))}
          {cards.map((c) => (
            <Card key={c.id} c={c} current={c.id} />
          ))}
        </Section>
      ) : null}
      <Block title={t('progress')} open={false}>
        <Progress d={d} />
      </Block>
      <Problem d={d} shown={shown} />
      <DesignBlock shown={shown} d={d} />
      <AcceptanceBlock d={d} shown={shown} />
      <Changes shown={shown} />
      <Others shown={shown} />
      {d.depends_on.length > 0 || d.depended_on_by.length > 0 ? (
        <div className="two section-apart">
          {d.depends_on.length > 0 ? (
            <Section title={t('depends_on')} open>
              <Linked list={d.depends_on} pair={(o) => [i.id, o]} />
            </Section>
          ) : null}
          {d.depended_on_by.length > 0 ? (
            <Section title={t('depended_on_by')} open>
              <Linked list={d.depended_on_by} pair={(o) => [o, i.id]} />
            </Section>
          ) : null}
        </div>
      ) : null}
      <History d={d} cards={earlierCards} asks={earlierAsks} />
    </HeadingLevel.Provider>
  );
}

/// An item's page, on the items view beside the tree.
export function Item({ id }: { id: number }) {
  const item = useResource<ItemDetail>(`item:${id}`, `/api/v1/items/${id}`);
  // Opening an item new to the owner marks it seen; the items list is read again when the interface reports it, and the
  // mark goes. A failed request leaves the item new, and the next time it is opened asks again.
  const unseen = item.data?.item.unseen === true;
  useEffect(() => {
    if (unseen) postJson(`/api/v1/items/${id}/seen`, {}).catch(() => undefined);
  }, [id, unseen]);
  return (
    <div className="page item-page">
      <Loaded r={item}>{(d) => <Detail d={d} />}</Loaded>
    </div>
  );
}
