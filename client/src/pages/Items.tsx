// The items: on the left, each project's items as a tree of what depends on what, with the buttons that show or hide
// the items in each state, each in its state's colour; on the right, the item picked, in place, or while none is, the
// first the tree shows. The split between them is Primer's PageLayout with a pane the owner drags, its width kept in
// this browser. Each project's tree is Primer's TreeView: a row says the item's number and title, what it needs from
// the owner, the items it waits for before it starts, and at its end, as muted words joined by " · ", the kind of the
// dependency by which the item above depends on it, how many of what it depends on are done, its place in the queue
// and its work's stage in words where the row has room, how far through the stages on hovering; then its state, as
// the chip a state is everywhere.

import { CheckIcon, LinkIcon } from '@primer/octicons-react';
import { Button, Heading, PageLayout, TreeView } from '@primer/react';
import { Fragment, type ReactNode, useState } from 'react';

import type { ItemLink, Items as ItemsData, ItemState, ItemSummary } from '../api';
import { DependencyKind, ITEM_TONE, ItemBadge, Loaded, StateChip } from '../components';
import { depsDone, firstShown, stageAt, STATES, tree, type TreeRow } from '../item/tree';
import { words } from '../labelled';
import { useResource } from '../live';
import { navigate } from '../route';
import { t, tn } from '../text';
import { joined } from '../voices';
import { Item } from './Item';

/// The states shown until their button says otherwise: what is settled is hidden.
const SHOWN: ItemState[] = ['working', 'queued', 'new', 'stopped'];

/// A value the owner set on this view, kept in this browser where it lets the page keep it.
function kept<T>(key: string, fallback: T, valid: (v: unknown) => v is T): T {
  try {
    const raw = window.localStorage.getItem(`wf-agents.${key}`);
    if (raw === null) return fallback;
    const v: unknown = JSON.parse(raw);
    return valid(v) ? v : fallback;
  } catch {
    return fallback;
  }
}

function keep(key: string, value: unknown) {
  try {
    window.localStorage.setItem(`wf-agents.${key}`, JSON.stringify(value));
  } catch {
    // Kept only where the browser lets it be.
  }
}

const isStates = (v: unknown): v is ItemState[] =>
  Array.isArray(v) && v.every((s) => typeof s === 'string' && (STATES as string[]).includes(s));

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

/// The end of a row, inside its trailing visual: its facts as muted words joined by " · ", the stage last, since it
/// shows only where the row has room, then the item's state as its chip.
function RowEnd({ facts, stage, state }: { facts: Array<ReactNode | null>; stage: ReactNode | null; state: ItemState }) {
  const shown = joined(facts);
  return (
    <span className="tree-end">
      {shown.length > 0 || stage !== null ? (
        <span className="muted">
          {shown.map((x, k) => (
            <Fragment key={k}>{x}</Fragment>
          ))}
          {stage !== null ? (
            <span className="tree-stage">
              {shown.length > 0 ? ' · ' : null}
              {stage}
            </span>
          ) : null}
        </span>
      ) : null}
      <ItemBadge state={state} />
    </span>
  );
}

function Row({ row, path, data, place, selected, folded, onFold }: RowProps) {
  const i = row.item;
  const open = () => navigate(`/items/${i.id}`);
  if (!row.full) {
    return (
      <TreeView.Item id={path} className="tree-row tree-ref" current={i.id === selected} onSelect={open}>
        <TreeView.LeadingVisual label={t('items_ref_title')}>
          <LinkIcon />
        </TreeView.LeadingVisual>
        <span className="muted" title={t('items_ref_title')}>
          #{i.id} {i.title}
        </span>
        <TreeView.TrailingVisual>
          <RowEnd facts={[row.kind !== null ? <DependencyKind kind={row.kind} /> : null]} stage={null} state={i.state} />
        </TreeView.TrailingVisual>
      </TreeView.Item>
    );
  }
  const deps = depsDone(i.id, data.items, data.links);
  const stage = i.state === 'done' || i.state === 'cancelled' ? null : stageAt(i.stage, i.stages);
  return (
    <TreeView.Item
      id={path}
      className="tree-row"
      current={i.id === selected}
      onSelect={open}
      expanded={row.children.length > 0 ? !folded.has(path) : null}
      onExpandedChange={(expanded) => onFold(path, expanded)}
    >
      <span className="tree-line">
        <span className="muted">#{i.id}</span> <span className="tree-title">{i.title}</span>
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
      <TreeView.TrailingVisual>
        <RowEnd
          facts={[
            row.kind !== null ? <DependencyKind kind={row.kind} /> : null,
            deps.of > 0 ? (
              <span title={tn('items_deps_title', deps.of, { done: deps.done })}>
                {deps.done}/{deps.of}
              </span>
            ) : null,
            place.has(i.id) ? <span title={t('items_queue_title')}>{t('items_queue', { n: place.get(i.id) ?? 0 })}</span> : null,
          ]}
          stage={
            stage !== null && i.stage !== null ? <span title={t('items_stage_title', stage)}>{words(i.stage)}</span> : null
          }
          state={i.state}
        />
      </TreeView.TrailingVisual>
      {row.children.length > 0 ? (
        <TreeView.SubTree>
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
        </TreeView.SubTree>
      ) : null}
    </TreeView.Item>
  );
}

/// One project: its heading, the buttons that show or hide each state's items with their counts, each in its state's
/// colour, filled as the state's chip while its items show, outlined in it while they hide; and its tree.
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
        <span className="tree-states">
          {STATES.map((s) => {
            const n = props.items.filter((i) => i.state === s).length;
            if (n === 0) return null;
            const on = props.shown.includes(s);
            return (
              <Button
                key={s}
                size="small"
                variant="invisible"
                className={`tree-state tree-state-${ITEM_TONE[s]}`}
                leadingVisual={on ? CheckIcon : undefined}
                count={n}
                aria-pressed={on}
                title={t(on ? 'items_hide' : 'items_show', { state: t(`item_${s}`) })}
                onClick={() => props.onShown(s)}
              >
                {t(`item_${s}`)}
              </Button>
            );
          })}
        </span>
      </div>
      {rows.length === 0 ? <p className="muted tree-none">{t('items_none_shown')}</p> : null}
      {rows.length > 0 ? (
        <TreeView aria-labelledby={heading}>
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
        </TreeView>
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
  const [shown, setShown] = useState<ItemState[]>(() => kept('items-states', SHOWN, isStates));
  const toggle = (s: ItemState) => {
    const next = shown.includes(s) ? shown.filter((x) => x !== s) : [...shown, s];
    setShown(next);
    keep('items-states', next);
  };
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
