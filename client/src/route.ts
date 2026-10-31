// The page's addresses: each view has a path, so it can be reloaded, linked and gone back to.

import { Link as PrimerLink } from '@primer/react';
import { createElement, useMemo, useSyncExternalStore, type MouseEvent, type ReactNode } from 'react';

import { roundName } from './format';

/// What the work's panel beside its chart shows: its rounds, its design cards, its helpers' talk with its engineer, or its
/// usage.
export type WorkTab = 'rounds' | 'cards' | 'helpers' | 'usage';

/// Which of a work's rounds its list shows besides all of them: those whose return was sent back, or those whose run
/// failed or was cut off.
export type RoundsOnly = 'sent_back' | 'failed';

export type WorkRoute = {
  page: 'work';
  id: number;
  /// The round open, by its number in the work; the address names it as `?round=R<work>-<n>`.
  round: number | null;
  cell: string | null;
  tab: WorkTab;
  only: RoundsOnly | null;
};

/// What the architect's panel over its chart shows: its rounds, across all its works, the item changes its batches made,
/// or its usage.
export const ARCHITECT_TABS = ['rounds', 'changes', 'usage'] as const;
export type ArchitectTab = (typeof ARCHITECT_TABS)[number];

/// What the panel beside the one-to-one with an architect shows: its notebook, its policy, or its queue.
export const SIDE_TABS = ['notebook', 'policy', 'queue'] as const;
export type SideTab = (typeof SIDE_TABS)[number];

/// The time the architect's usage counts over, back from now: the last day, the last 7 days (the default), the last 30
/// days, or all time.
export const SPANS = ['1d', '7d', '30d', 'all'] as const;
export type Span = (typeof SPANS)[number];

/// A round, by its work and its number in the work; the address names it as `R<work>-<n>`.
export type RoundKey = { work: number; n: number };

/// The round a work and a number name, when both are given.
export const roundAt = (work: number | null | undefined, n: number | null | undefined): RoundKey | null =>
  work != null && n != null ? { work, n } : null;

/// Whether two rounds are one.
export const sameRound = (a: RoundKey | null, b: RoundKey | null): boolean =>
  a !== null && b !== null && a.work === b.work && a.n === b.n;

export type ArchitectRoute = {
  page: 'architect';
  /// The project whose architect the page shows; with none, the configuration's one project.
  project: string | null;
  tab: ArchitectTab;
  /// The round open, of any of the architect's works: its list opens on it, centred, and on the newest with none.
  round: RoundKey | null;
  cell: string | null;
  /// The time its usage counts over.
  span: Span;
};

/// The observations, every project's or one project's, and the one open beside them, if one is.
export type ObservationsRoute = {
  page: 'observations';
  /// The project whose observations alone the view shows; with none, every project's.
  project: string | null;
  /// The observation open, by its number; with none, the list fills the view.
  open: number | null;
};

/// The period the System view counts what the system did over: the week under way, the last four weeks with it, or all
/// time.
export const PERIODS = ['week', '4w', 'all'] as const;
export type Period = (typeof PERIODS)[number];

export type Route =
  | { page: 'works' }
  | WorkRoute
  | ArchitectRoute
  | { page: 'items' }
  | { page: 'item'; id: number }
  | ObservationsRoute
  | {
      page: 'messages';
      /// The conversation open, by its key; with none, the first in the list.
      open: string | null;
      /// The tab of the panel beside the one-to-one with an architect that the address opens the panel on; with none,
      /// the panel is as the owner left it, on its notebook.
      side?: SideTab;
    }
  | {
      page: 'system';
      /// The period the view counts over.
      period: Period;
    }
  | { page: 'settings' }
  | {
      page: 'flow';
      /// The stored version shown; with none, the one new works start on.
      version: number | null;
      /// The graph of the cell open: two graphs may each have a cell of one name.
      graph: string | null;
      cell: string | null;
      /// Instead of a stored version, the flow at a git ref of a project's checkout.
      project: string | null;
      ref: string | null;
      /// The stored version the shown flow is compared with.
      from: number | null;
    }
  | { page: 'missing' };

const num = (s: string | null | undefined): number | null => {
  if (s == null || !/^\d+$/.test(s)) return null;
  return Number(s);
};

/// The round a name such as `R5-9` names: work 5's ninth.
export const roundKey = (s: string | null): RoundKey | null => {
  const m = s === null ? null : /^R(\d+)-(\d+)$/.exec(s);
  return m !== null ? { work: Number(m[1]), n: Number(m[2]) } : null;
};

/// The number in the work of the round a name such as `R5-9` names, when it is a round of that work.
const roundOf = (work: number, s: string | null): number | null => {
  const k = roundKey(s);
  return k !== null && k.work === work ? k.n : null;
};

export function parse(pathname: string, search: string): Route {
  const q = new URLSearchParams(search);
  const parts = pathname.split('/').filter((p) => p !== '');
  const [first, second] = parts;
  if (parts.length === 0 || (first === 'works' && parts.length === 1)) return { page: 'works' };
  if (first === 'works' && parts.length === 2) {
    const id = num(second);
    if (id !== null) {
      const tab = q.get('tab');
      const only = q.get('only');
      return {
        page: 'work',
        id,
        round: roundOf(id, q.get('round')),
        cell: q.get('cell'),
        tab: tab === 'cards' || tab === 'helpers' || tab === 'usage' ? tab : 'rounds',
        only: only === 'sent_back' || only === 'failed' ? only : null,
      };
    }
  }
  if (first === 'architect' && parts.length === 1) {
    const tab = ARCHITECT_TABS.find((x) => x === q.get('tab')) ?? 'rounds';
    return {
      page: 'architect',
      project: q.get('project'),
      tab,
      round: roundKey(q.get('round')),
      cell: q.get('cell'),
      span: SPANS.find((x) => x === q.get('span')) ?? '7d',
    };
  }
  if (first === 'items' && parts.length === 1) return { page: 'items' };
  if (first === 'observations' && parts.length <= 2) {
    const open = parts.length === 2 ? num(second) : null;
    if (parts.length === 1 || open !== null) return { page: 'observations', project: q.get('project'), open };
  }
  if (first === 'messages' && parts.length === 1) {
    const side = SIDE_TABS.find((x) => x === q.get('side'));
    return side !== undefined ? { page: 'messages', open: q.get('c'), side } : { page: 'messages', open: q.get('c') };
  }
  // The System view was the Spend view, whose address still leads to it.
  if ((first === 'system' || first === 'spend') && parts.length === 1) {
    return { page: 'system', period: PERIODS.find((p) => p === q.get('period')) ?? 'week' };
  }
  if (first === 'settings' && parts.length === 1) return { page: 'settings' };
  if (first === 'items' && parts.length === 2) {
    const id = num(second);
    if (id !== null) return { page: 'item', id };
  }
  if (first === 'flow' && parts.length <= 2) {
    const version = parts.length === 2 ? num(second) : null;
    if (parts.length === 1 || version !== null) {
      return {
        page: 'flow',
        version,
        graph: q.get('graph'),
        cell: q.get('cell'),
        project: q.get('project'),
        ref: q.get('ref'),
        from: num(q.get('from')),
      };
    }
  }
  return { page: 'missing' };
}

export function href(r: Route): string {
  const query = (pairs: Array<[string, string | number | null]>) => {
    const q = new URLSearchParams();
    for (const [k, v] of pairs) if (v !== null) q.set(k, String(v));
    const s = q.toString();
    return s ? `?${s}` : '';
  };
  switch (r.page) {
    case 'works':
      return '/works';
    case 'work':
      return `/works/${r.id}${query([
        ['tab', r.tab === 'rounds' ? null : r.tab],
        ['only', r.only],
        ['round', r.round === null ? null : roundName(r.id, r.round)],
        ['cell', r.cell],
      ])}`;
    case 'architect':
      return `/architect${query([
        ['project', r.project],
        ['tab', r.tab === 'rounds' ? null : r.tab],
        ['span', r.span === '7d' ? null : r.span],
        ['round', r.round !== null ? roundName(r.round.work, r.round.n) : null],
        ['cell', r.cell],
      ])}`;
    case 'items':
      return '/items';
    case 'item':
      return `/items/${r.id}`;
    case 'observations':
      return `/observations${r.open === null ? '' : `/${r.open}`}${query([['project', r.project]])}`;
    case 'messages':
      return `/messages${query([
        ['c', r.open],
        ['side', r.side ?? null],
      ])}`;
    case 'system':
      return `/system${query([['period', r.period === 'week' ? null : r.period]])}`;
    case 'settings':
      return '/settings';
    case 'flow':
      return `/flow${r.version === null ? '' : `/${r.version}`}${query([
        ['project', r.project],
        ['ref', r.ref],
        ['from', r.from],
        ['graph', r.graph],
        ['cell', r.cell],
      ])}`;
    case 'missing':
      return '/';
  }
}

/// The address of a work's page: its rounds with nothing open, or what `more` names.
export function workHref(id: number, more: Partial<Omit<WorkRoute, 'page' | 'id'>> = {}): string {
  return href({ page: 'work', id, round: null, cell: null, tab: 'rounds', only: null, ...more });
}

/// The address of an architect's page: its rounds, opened on the newest, or what `more` names.
export function architectHref(more: Partial<Omit<ArchitectRoute, 'page'>> = {}): string {
  return href({
    page: 'architect',
    project: null,
    tab: 'rounds',
    round: null,
    cell: null,
    span: '7d',
    ...more,
  });
}

/// The address of the observations: every project's, or with `project` that project's alone; with `open`, that
/// observation open beside them.
export const observationsHref = (project: string | null = null, open: number | null = null): string =>
  href({ page: 'observations', project, open });

/// The address of the one-to-one with a project's architect with the panel beside it open on a tab.
export const sideHref = (project: string, side: SideTab): string =>
  href({ page: 'messages', open: `architect:${project}`, side });

/// A path of the interface about a project's architect: for the project named, or with none, the configuration's one
/// project.
export function forProject(path: string, project: string | null): string {
  return project !== null ? `${path}?project=${encodeURIComponent(project)}` : path;
}

const NAVIGATED = 'wf-navigate';

export function navigate(to: string, replace = false): void {
  if (to === location.pathname + location.search) return;
  if (replace) history.replaceState(null, '', to);
  else history.pushState(null, '', to);
  window.dispatchEvent(new Event(NAVIGATED));
}

function subscribe(changed: () => void): () => void {
  window.addEventListener('popstate', changed);
  window.addEventListener(NAVIGATED, changed);
  return () => {
    window.removeEventListener('popstate', changed);
    window.removeEventListener(NAVIGATED, changed);
  };
}

const where = () => location.pathname + location.search;

export function useRoute(): Route {
  const at = useSyncExternalStore(subscribe, where);
  return useMemo(() => {
    const i = at.indexOf('?');
    return i < 0 ? parse(at, '') : parse(at.slice(0, i), at.slice(i));
  }, [at]);
}

/// A link that moves within the page without loading it again, drawn as Primer's Link; a click with a modifier key
/// opens it as the browser would. The click stays with the link, so a link in a row that opens something else when
/// clicked opens only its own.
export function Link(props: { to: string; className?: string; title?: string; children: ReactNode }) {
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    e.stopPropagation();
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    navigate(props.to);
  };
  return createElement(
    PrimerLink,
    { href: props.to, className: props.className, title: props.title, onClick },
    props.children,
  );
}
