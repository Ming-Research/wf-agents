// The items: on the left, each project's items as a tree of what depends on what, with chips that show or hide the
// items in each state; on the right, the item picked, in place. The split between them is dragged, and kept in this
// browser. A row says the item's state by the colour of its edge, its number and title, what it needs from the owner,
// its work's stage as a bar behind it, how many of what it depends on are done, and its place in the queue.

import { useState, type PointerEvent as ReactPointerEvent } from 'react';

import type { ItemLink, Items as ItemsData, ItemState, ItemSummary } from '../api';
import { Loaded } from '../components';
import { depsDone, stageShare, STATES, tree, type TreeRow } from '../item/tree';
import { words } from '../labelled';
import { useResource } from '../live';
import { navigate } from '../route';
import { t } from '../text';
import { Item } from './Item';

/// The states shown until their chip says otherwise: what is settled is hidden.
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
const isWidth = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= 240;

type RowProps = {
  row: TreeRow;
  depth: number;
  data: ItemsData;
  place: Map<number, number>;
  selected: number | null;
  folded: Set<number>;
  onFold: (id: number) => void;
};

function Row({ row, depth, data, place, selected, folded, onFold }: RowProps) {
  const i = row.item;
  const pad = { paddingLeft: 8 + depth * 18 };
  if (!row.full) {
    return (
      <div
        className={`tree-row tree-ref item-edge-${i.state}${i.id === selected ? ' selected' : ''}`}
        style={pad}
        onClick={() => navigate(`/items/${i.id}`)}
        title={t('items_ref_title')}
      >
        <span className="tree-fold" />
        <span className="muted">↳ #{i.id}</span> <span className="tree-title muted">{i.title}</span>
      </div>
    );
  }
  const deps = depsDone(i.id, data.items, data.links);
  const share = i.state === 'done' || i.state === 'cancelled' ? null : stageShare(i.stage, i.stages);
  const open = !folded.has(i.id);
  const style =
    share !== null ? { ...pad, backgroundSize: `${Math.round(share * 100)}% 100%` } : pad;
  return (
    <>
      <div
        className={`tree-row item-edge-${i.state}${share !== null ? ' tree-staged' : ''}${i.id === selected ? ' selected' : ''}`}
        style={style}
        onClick={() => navigate(`/items/${i.id}`)}
      >
        {row.children.length > 0 ? (
          <button
            type="button"
            className="tree-fold"
            aria-expanded={open}
            title={t(open ? 'items_fold' : 'items_unfold')}
            onClick={(e) => {
              e.stopPropagation();
              onFold(i.id);
            }}
          >
            {open ? '▾' : '▸'}
          </button>
        ) : (
          <span className="tree-fold" />
        )}
        <span className="muted">#{i.id}</span> <span className="tree-title">{i.title}</span>
        {i.open_design_cards > 0 ? (
          <span className="chip warn">{t('open_design_cards_chip', { n: i.open_design_cards })}</span>
        ) : null}
        {i.open_asks > 0 ? <span className="chip warn">{t('open_asks_chip', { n: i.open_asks })}</span> : null}
        <span className="tree-facts">
          {share !== null && i.stage !== null ? (
            <span
              className="muted"
              title={t('items_stage_title', { at: i.stages.indexOf(i.stage) + 1, of: i.stages.length })}
            >
              {words(i.stage)}
            </span>
          ) : null}
          {deps.of > 0 ? (
            <span className={deps.done === deps.of ? 'good' : 'muted'} title={t('items_deps_title', deps)}>
              {deps.done}/{deps.of}
            </span>
          ) : null}
          {place.has(i.id) ? (
            <span className="muted" title={t('items_queue_title')}>
              {t('items_queue', { n: place.get(i.id) ?? 0 })}
            </span>
          ) : null}
        </span>
      </div>
      {open
        ? row.children.map((c) => (
            <Row
              key={`${c.item.id}:${c.full ? 'full' : 'ref'}`}
              row={c}
              depth={depth + 1}
              data={data}
              place={place}
              selected={selected}
              folded={folded}
              onFold={onFold}
            />
          ))
        : null}
    </>
  );
}

/// One project: its heading, the chips that show or hide each state's items with their counts, and its tree.
function Project(props: {
  name: string;
  items: ItemSummary[];
  links: ItemLink[];
  data: ItemsData;
  place: Map<number, number>;
  shown: ItemState[];
  onShown: (s: ItemState) => void;
  selected: number | null;
  folded: Set<number>;
  onFold: (id: number) => void;
}) {
  const visible = props.items.filter((i) => props.shown.includes(i.state));
  const rows = tree(visible, props.links);
  return (
    <section className="tree-project">
      <div className="tree-head">
        <span className="works-label">{props.name}</span>
        <span className="tree-chips">
          {STATES.map((s) => {
            const n = props.items.filter((i) => i.state === s).length;
            if (n === 0) return null;
            const on = props.shown.includes(s);
            return (
              <button
                key={s}
                type="button"
                className={`state-chip item-${s}${on ? '' : ' state-chip-off'}`}
                aria-pressed={on}
                title={t(on ? 'items_hide' : 'items_show', { state: t(`item_${s}`) })}
                onClick={() => props.onShown(s)}
              >
                {t(`item_${s}`)} {n}
              </button>
            );
          })}
        </span>
      </div>
      {rows.length === 0 ? <p className="muted tree-none">{t('items_none_shown')}</p> : null}
      {rows.map((r) => (
        <Row
          key={r.item.id}
          row={r}
          depth={0}
          data={props.data}
          place={props.place}
          selected={props.selected}
          folded={props.folded}
          onFold={props.onFold}
        />
      ))}
    </section>
  );
}

function Tree({ data, selected }: { data: ItemsData; selected: number | null }) {
  const [shown, setShown] = useState<ItemState[]>(() => kept('items-states', SHOWN, isStates));
  const [folded, setFolded] = useState<Set<number>>(() => new Set());
  if (data.items.length === 0) return <p className="muted tree-none">{t('no_items')}</p>;
  const projects = [...new Set(data.items.map((i) => i.project))].sort();
  // The queue is one for every project: an item's place in it counts from 1.
  const place = new Map(
    data.items
      .filter((i) => i.queued !== null)
      .sort((a, b) => (a.queued ?? 0) - (b.queued ?? 0))
      .map((i, n) => [i.id, n + 1]),
  );
  const toggle = (s: ItemState) => {
    const next = shown.includes(s) ? shown.filter((x) => x !== s) : [...shown, s];
    setShown(next);
    keep('items-states', next);
  };
  const fold = (id: number) =>
    setFolded((f) => {
      const next = new Set(f);
      if (next.has(id)) next.delete(id);
      else next.add(id);
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
          onShown={toggle}
          selected={selected}
          folded={folded}
          onFold={fold}
        />
      ))}
    </>
  );
}

/// The width of the tree at first.
const WIDTH = 520;

export function Items({ selected }: { selected: number | null }) {
  const items = useResource<ItemsData>('items', '/api/v1/items');
  const [width, setWidth] = useState(() => kept('items-width', WIDTH, isWidth));
  const [from, setFrom] = useState<{ x: number; w: number } | null>(null);
  const grip = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    setFrom({ x: e.clientX, w: width });
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (from === null) return;
    setWidth(Math.max(240, Math.min(window.innerWidth - 320, from.w + e.clientX - from.x)));
  };
  const up = () => {
    if (from === null) return;
    setFrom(null);
    keep('items-width', width);
  };
  return (
    <div className="items-screen">
      <div className="items-tree" style={{ width }}>
        <Loaded r={items}>{(data) => <Tree data={data} selected={selected} />}</Loaded>
      </div>
      <div
        className={`items-grip${from !== null ? ' items-grip-on' : ''}`}
        role="separator"
        aria-orientation="vertical"
        title={t('items_drag')}
        onPointerDown={grip}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
      />
      <div className="items-item">
        {selected !== null ? <Item id={selected} /> : <p className="muted items-pick">{t('items_pick')}</p>}
      </div>
    </div>
  );
}
