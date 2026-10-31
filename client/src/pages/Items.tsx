// The items: on the left, each project's items as a tree of what depends on what, with the buttons that show or hide
// the items in each state, each in its state's colour; on the right, the item picked, in place, or while none is, the
// first the tree shows. The split between them is Primer's PageLayout with a pane the owner drags, its width kept in
// this browser. Each project's tree is ours, after the v1 board's: each item a card whose left edge is in its state's
// colour, the items it depends on in a list under it, set in, with a rule down their left. A row has its toggle, which
// folds the rows under it; the item's number, of a fixed width, tinted in its state's colour and saying its state on
// hovering; its title; what it needs from the owner and the items it waits for before it starts; and at its end, as
// muted words joined by a bar, the kind of the dependency by which the item above depends on it, how many of what it
// depends on are done, its place in the queue and its work's stage in words where the row has room, how far through
// the stages on hovering. A click on a row opens its item; a double click on a row with rows under it folds or
// unfolds them, as its toggle does.

import { ChevronDownIcon, ChevronRightIcon, LinkIcon } from '@primer/octicons-react';
import { Heading, PageLayout } from '@primer/react';
import { Fragment, type MouseEvent, type ReactNode, useState } from 'react';

import type { ItemLink, Items as ItemsData, ItemState, ItemSummary } from '../api';
import { DependencyKind, ITEM_TONE, Loaded, StateChip, StateFilter, useShownStates } from '../components';
import { depsDone, firstShown, stageAt, STATES, tree, type TreeRow } from '../item/tree';
import { words } from '../labelled';
import { useResource } from '../live';
import { href, navigate } from '../route';
import { t, tn } from '../text';
import { joined, SEP } from '../voices';
import { Item } from './Item';

/// The states shown until their button says otherwise: what is settled is hidden.
const SHOWN: ItemState[] = ['working', 'queued', 'new', 'stopped'];

type RowProps = {
  row: TreeRow;
  /// The row's place in its tree, which names it among the rows: an item shown in several places is several rows.
  path: string;
  data: ItemsData;
  place: Map<number, number>;
  selected: number | null;
  folded: Set<string>;
  onFold: (path: string, open: boolean) => void;
};

/// The end of a row: its facts as muted words joined by a bar, the stage last, since it shows only where the row has
/// room; nothing for none.
function RowEnd({ facts, stage }: { facts: Array<ReactNode | null>; stage: ReactNode | null }) {
  const shown = joined(facts);
  if (shown.length === 0 && stage === null) return null;
  return (
    <span className="tree-end muted">
      {shown.map((x, k) => (
        <Fragment key={k}>{x}</Fragment>
      ))}
      {stage !== null ? (
        <span className="tree-stage">
          {shown.length > 0 ? SEP : null}
          {stage}
        </span>
      ) : null}
    </span>
  );
}

/// What an item needs from the owner, and the items it waits for before it starts, as chips; nothing for none.
function Needs({ i }: { i: ItemSummary }) {
  if (i.open_design_cards === 0 && i.open_asks === 0 && i.blocked_by.length === 0) return null;
  return (
    <span className="tree-needs">
      {i.open_design_cards > 0 ? (
        <StateChip tone="attention">{t('open_design_cards_chip', { n: i.open_design_cards })}</StateChip>
      ) : null}
      {i.open_asks > 0 ? <StateChip tone="attention">{t('open_asks_chip', { n: i.open_asks })}</StateChip> : null}
      {i.blocked_by.length > 0 ? (
        <StateChip tone="attention" title={t('items_blocked_title')}>
          {t('items_blocked', { items: i.blocked_by.map((n) => `#${n}`).join(', ') })}
        </StateChip>
      ) : null}
    </span>
  );
}

/// Whether a press landed on the row itself, rather than on its toggle, whose own clicks fold the row already.
function onRow(e: MouseEvent<HTMLElement>): boolean {
  return e.target instanceof Element && e.target.closest('.tree-toggle') === null;
}

function Row({ row, path, data, place, selected, folded, onFold }: RowProps) {
  const i = row.item;
  const parent = row.full && row.children.length > 0;
  const open = parent && !folded.has(path);
  const deps = depsDone(i.id, data.items, data.links);
  const stage = i.state === 'done' || i.state === 'cancelled' ? null : stageAt(i.stage, i.stages);
  return (
    <li>
      <div
        className={`tree-row tree-tone-${ITEM_TONE[i.state]}${i.id === selected ? ' tree-current' : ''}`}
        // A double click folds or unfolds the rows under this one, into the fold its toggle keeps, and selects no word;
        // on a row with none under it, it does what its clicks did, which open the item.
        onMouseDown={(e) => {
          if (parent && e.detail > 1 && onRow(e)) e.preventDefault();
        }}
        onDoubleClick={(e) => {
          if (parent && onRow(e)) onFold(path, !open);
        }}
      >
        {/* The toggle stands over the room the row leaves at its left, where a row with none under it opens its item. */}
        {parent ? (
          <button
            type="button"
            className="tree-toggle"
            aria-expanded={open}
            aria-label={t(open ? 'items_fold' : 'items_unfold')}
            onClick={() => onFold(path, !open)}
          >
            {open ? <ChevronDownIcon size={12} /> : <ChevronRightIcon size={12} />}
          </button>
        ) : null}
        {/* A button that opens the item in place, rather than a link, so that its words can be selected and copied as
            a row's could: a browser drags a link instead. */}
        <button
          type="button"
          className="tree-open"
          aria-current={i.id === selected ? 'true' : undefined}
          onClick={() => navigate(href({ page: 'item', id: i.id }))}
        >
          <span className="tree-num" title={t(`item_${i.state}`)}>
            #{i.id}
          </span>
          {row.full ? (
            <>
              <span className="tree-title" title={i.title}>
                {i.title}
              </span>
              <Needs i={i} />
              <RowEnd
                facts={[
                  row.kind !== null ? <DependencyKind kind={row.kind} /> : null,
                  deps.of > 0 ? (
                    <span title={tn('items_deps_title', deps.of, { done: deps.done })}>
                      {deps.done}/{deps.of}
                    </span>
                  ) : null,
                  place.has(i.id) ? (
                    <span title={t('items_queue_title')}>{t('items_queue', { n: place.get(i.id) ?? 0 })}</span>
                  ) : null,
                ]}
                stage={
                  stage !== null && i.stage !== null ? (
                    <span title={t('items_stage_title', stage)}>{words(i.stage)}</span>
                  ) : null
                }
              />
            </>
          ) : (
            <>
              <LinkIcon className="tree-ref-mark" aria-label={t('items_ref_title')} />
              <span className="tree-title muted" title={t('items_ref_title')}>
                {i.title}
              </span>
              <RowEnd facts={[row.kind !== null ? <DependencyKind kind={row.kind} /> : null]} stage={null} />
            </>
          )}
        </button>
      </div>
      {open ? (
        <ul className="tree">
          {row.children.map((c) => {
            const at = `${path}/${c.item.id}${c.full ? '' : 'r'}`;
            return (
              <Row
                key={at}
                row={c}
                path={at}
                data={data}
                place={place}
                selected={selected}
                folded={folded}
                onFold={onFold}
              />
            );
          })}
        </ul>
      ) : null}
    </li>
  );
}

/// One project: its heading, the buttons that show or hide each state's items with their counts (`StateFilter`, the one
/// part the list of conversations shows or hides each state's works with too); and its tree.
function Project(props: {
  name: string;
  items: ItemSummary[];
  links: ItemLink[];
  data: ItemsData;
  place: Map<number, number>;
  shown: ItemState[];
  onShown: (s: ItemState) => void;
  selected: number | null;
  folded: Set<string>;
  onFold: (path: string, open: boolean) => void;
}) {
  const visible = props.items.filter((i) => props.shown.includes(i.state));
  const rows = tree(visible, props.links);
  const heading = `items-project-${props.name}`;
  return (
    <section className="tree-project">
      <div className="tree-head">
        <Heading as="h2" variant="small" id={heading}>
          {props.name}
        </Heading>
        <StateFilter
          order={STATES}
          states={props.items.map((i) => i.state)}
          shown={props.shown}
          tone={ITEM_TONE}
          name={(s) => t(`item_${s}`)}
          title={(s, on) => t(on ? 'items_hide' : 'items_show', { state: t(`item_${s}`) })}
          onShown={props.onShown}
        />
      </div>
      {rows.length === 0 ? <p className="muted tree-none">{t('items_none_shown')}</p> : null}
      {rows.length > 0 ? (
        <ul className="tree" aria-labelledby={heading}>
          {rows.map((r) => {
            const at = `${props.name}/${r.item.id}`;
            return (
              <Row
                key={at}
                row={r}
                path={at}
                data={props.data}
                place={props.place}
                selected={props.selected}
                folded={props.folded}
                onFold={props.onFold}
              />
            );
          })}
        </ul>
      ) : null}
    </section>
  );
}

function Tree({ data, selected, shown, onShown }: {
  data: ItemsData;
  selected: number | null;
  shown: ItemState[];
  onShown: (s: ItemState) => void;
}) {
  // The rows folded, by their place in the tree.
  const [folded, setFolded] = useState<Set<string>>(() => new Set());
  if (data.items.length === 0) return <p className="muted tree-none">{t('no_items')}</p>;
  const projects = [...new Set(data.items.map((i) => i.project))].sort();
  // The queue is one for every project: an item's place in it counts from 1.
  const place = new Map(
    data.items
      .filter((i) => i.queued !== null)
      .sort((a, b) => (a.queued ?? 0) - (b.queued ?? 0))
      .map((i, n) => [i.id, n + 1]),
  );
  const fold = (path: string, open: boolean) =>
    setFolded((f) => {
      const next = new Set(f);
      if (open) next.delete(path);
      else next.add(path);
      return next;
    });
  return (
    <>
      {projects.map((p) => (
        <Project
          key={p}
          name={p}
          items={data.items.filter((i) => i.project === p)}
          links={data.links}
          data={data}
          place={place}
          shown={shown}
          onShown={onShown}
          selected={selected}
          folded={folded}
          onFold={fold}
        />
      ))}
    </>
  );
}

export function Items({ selected }: { selected: number | null }) {
  const items = useResource<ItemsData>('items', '/api/v1/items');
  const [shown, toggle] = useShownStates('items-states', STATES, SHOWN);
  // While no item is picked, the first the tree shows is open.
  const open = selected ?? (items.data !== undefined ? firstShown(items.data.items, items.data.links, shown) : null);
  return (
    <PageLayout containerWidth="full" padding="none" rowGap="none" columnGap="none" className="items-screen">
      {/* The pane stays under the bar, which is as tall as Primer's UnderlineNav, while the item scrolls the page. */}
      <PageLayout.Pane
        position="start"
        resizable
        widthStorageKey="wf-agents.items-pane"
        width={{ min: '240px', default: '520px', max: '1200px' }}
        divider="line"
        padding="none"
        sticky
        offsetHeader={48}
        className="items-tree"
        aria-label={t('nav_items')}
      >
        <div className="items-trees">
          <Loaded r={items}>{(data) => <Tree data={data} selected={open} shown={shown} onShown={toggle} />}</Loaded>
        </div>
      </PageLayout.Pane>
      <PageLayout.Content padding="none" width="full" className="items-item">
        {open !== null ? <Item id={open} /> : null}
      </PageLayout.Content>
    </PageLayout>
  );
}
