// The window floating over the right side of a chart, the flow's, a work's and an architect's alike: on a work's page
// and an architect's the page's panel in tabs on top of it, and the panel of what is open on the chart under them, or
// alone. It covers the chart and never moves it: opening it, minimising it, expanding it or dragging its width lays
// nothing out again, and a cell the chart centres on is centred in the part of the chart the window leaves.
// Minimised, it folds to its one head row at the chart's top right: what is open's title, or else the tabs with their
// counts; a click on the row, a tab, a click on the chart that opens something or the button at the row's right end
// expands it to where it was. It starts minimised each time a chart page opens; its width is kept in this browser, one
// for every page.

import { ChevronDownIcon, ChevronUpIcon } from '@primer/octicons-react';
import { IconButton } from '@primer/react';
import {
  createContext,
  type MouseEvent,
  type ReactNode,
  type Ref,
  type RefObject,
  useCallback,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';

import { t } from '../text';
import { Grip, savedSize } from './Panel';

const WIDTH_KEY = 'wf-agents.dock-width';
const WIDTH_MIN = 320;

/// The least width of the chart the expanded window leaves that is room to centre a cell in. Where it leaves less, as
/// at a phone's width, where it takes the chart's whole width, the chart centres as if nothing covered it.
export const LEAST_SEEN = 120;

/// How much of the chart's right side the window covers, in pixels: from the window's left edge to the chart's right
/// edge while it is expanded (`dock`); none while it is minimised or not there, or when what it leaves is narrower than
/// `LEAST_SEEN`.
export function coverOf(chart: { left: number; right: number } | null, dock: { left: number } | null): number {
  if (chart === null || dock === null) return 0;
  const cover = Math.max(0, chart.right - dock.left);
  return chart.right - chart.left - cover >= LEAST_SEEN ? cover : 0;
}

/// Which head is the window's top row, the row it folds to, holding at its right end the button that minimises and
/// expands it: the page's tabs, on a page with a panel of its own, except while the window is minimised with something
/// open; else the head of what is open.
export function foldIn(tabs: boolean, open: boolean, minimised: boolean): 'tabs' | 'open' {
  return tabs && !(minimised && open) ? 'tabs' : 'open';
}

/// What folds the window from its top row, minimising it or expanding it: the button at the row's right end, and a
/// click on the row (`foldOnClick`).
export type Fold = { button: ReactNode; onRow: (e: MouseEvent<HTMLElement>) => void };

/// What the page's panel in tabs needs of the window it is in: what folds the window while its row is the window's top
/// row, and the window's expanding, which a tab picked does.
export type DockHead = { fold: Fold | null; expand: () => void };

export const DockContext = createContext<DockHead | null>(null);

/// The window the page's panel is in, if it is in one.
export function useDockHead(): DockHead | null {
  return useContext(DockContext);
}

/// The button at the right end of the window's top row: minimises the window, or expands it, saying which in its label
/// and its chevron; with no `aria-expanded`, which Primer draws as a menu held open.
export function FoldButton({ minimised, onFold }: { minimised: boolean; onFold: () => void }) {
  return (
    <IconButton
      size="small"
      variant="invisible"
      icon={minimised ? ChevronDownIcon : ChevronUpIcon}
      aria-label={t(minimised ? 'dock_expand' : 'dock_minimise')}
      onClick={onFold}
    />
  );
}

/// The controls a top row may hold, each doing what it does when clicked instead of folding the window.
const CONTROLS = 'a, button, input, select, textarea, [role="button"], [role="tab"]';

/// Whether a click on the window's top row (`row`) folds the window: not one on a tab, a link or a button in the row
/// (`target` is what was clicked), which does what it does, and not one that ends a drag selecting some of the row's
/// text (`selection`), which stays selected.
export function foldsOn(
  row: { contains: (other: Node | null) => boolean },
  target: { closest: (selector: string) => Element | null } | null,
  selection: { isCollapsed: boolean; anchorNode: Node | null } | null,
): boolean {
  const control = target?.closest(CONTROLS) ?? null;
  if (control !== null && row.contains(control)) return false;
  return selection === null || selection.isCollapsed || !row.contains(selection.anchorNode);
}

/// The click on the window's top row that minimises the window or expands it, as the button at the row's end does.
export function foldOnClick(toggle: () => void) {
  return (e: MouseEvent<HTMLElement>) => {
    if (foldsOn(e.currentTarget, e.target instanceof Element ? e.target : null, window.getSelection())) toggle();
  };
}

/// Whether the window `dock` is minimised, which it is as the page opens, and the two ways to change it. Expanding
/// brings each part of the window back to where it was scrolled to when it was minimised.
export function useMinimised(dock: RefObject<HTMLElement | null>) {
  const [minimised, setMinimised] = useState(true);
  const scrolled = useRef(new Map<Element, number>());
  const minimise = useCallback(() => {
    const parts = dock.current?.querySelectorAll('.drawer-body, .panel-body') ?? [];
    scrolled.current = new Map([...parts].map((el) => [el, el.scrollTop]));
    setMinimised(true);
  }, [dock]);
  const expand = useCallback(() => setMinimised(false), []);
  useLayoutEffect(() => {
    if (minimised) return;
    for (const [el, top] of scrolled.current) {
      if (el.isConnected) el.scrollTop = top;
    }
    scrolled.current = new Map();
  }, [minimised]);
  return { minimised, minimise, expand };
}

/// The window: its left edge drags its width, and it floats at the chart's full height but for the room round it, or,
/// minimised, at its head row's height. On a work's page and an architect's it is half the chart's width until the
/// owner drags it.
export function Dock({ half, minimised, children, ref }: {
  half: boolean;
  minimised: boolean;
  children: ReactNode;
  ref?: Ref<HTMLElement>;
}) {
  const [width, setWidth] = useState<number | null>(() => savedSize(WIDTH_KEY, WIDTH_MIN));
  return (
    <aside
      ref={ref}
      className={`dock${half ? ' dock-half' : ''}${minimised ? ' dock-min' : ''}`}
      style={width !== null ? { width } : undefined}
    >
      <Grip edge="left" min={WIDTH_MIN} storage={WIDTH_KEY} label={t('dock_grip')} onSize={setWidth} />
      <div className="dock-body">{children}</div>
    </aside>
  );
}
