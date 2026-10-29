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

/// What the architect's panel beside its chart shows: the rounds of the stretch shown, the item changes its batches made,
/// the project's observations, what to do next, or the policy in force.
export const ARCHITECT_TABS = ['rounds', 'changes', 'observations', 'next', 'policy'] as const;
export type ArchitectTab = (typeof ARCHITECT_TABS)[number];

export type ArchitectRoute = {
  page: 'architect';
  /// The project whose architect the page shows; with none, the configuration's one project.
  project: string | null;
  /// The wake shown, by its work; with none, the wake running, else the latest.
  wake: number | null;
  tab: ArchitectTab;
  /// The round of the wake shown that is open, by its number in the wake; only with a wake named.
  round: number | null;
  cell: string | null;
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
  | {
      page: 'messages';
      /// The conversation open, by its key; with none, the first in the list.
      open: string | null;
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

/// The number in the work of the round a name such as `R5-9` names, when it is a round of that work.
const roundOf = (work: number, s: string | null): number | null => {
  const m = s === null ? null : /^R(\d+)-(\d+)$/.exec(s);
  return m !== null && Number(m[1]) === work ? Number(m[2]) : null;
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
    const wake = num(q.get('wake'));
    const tab = ARCHITECT_TABS.find((x) => x === q.get('tab')) ?? 'rounds';
    return {
      page: 'architect',
      project: q.get('project'),
      wake,
      tab,
      round: wake !== null ? roundOf(wake, q.get('round')) : null,
      cell: q.get('cell'),
    };
  }
  if (first === 'items' && parts.length === 1) return { page: 'items' };
  if (first === 'messages' && parts.length === 1) return { page: 'messages', open: q.get('c') };
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
        ['wake', r.wake],
        ['tab', r.tab === 'rounds' ? null : r.tab],
        ['round', r.wake !== null && r.round !== null ? roundName(r.wake, r.round) : null],
        ['cell', r.cell],
      ])}`;
    case 'items':
      return '/items';
    case 'item':
      return `/items/${r.id}`;
    case 'messages':
      return `/messages${query([['c', r.open]])}`;
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

/// The address of an architect's page: the wake running or the latest, on its asks, or what `more` names.
export function architectHref(more: Partial<Omit<ArchitectRoute, 'page'>> = {}): string {
  return href({ page: 'architect', project: null, wake: null, tab: 'rounds', round: null, cell: null, ...more });
}

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

/// A link that moves within the page without loading it again, drawn as Primer's Link (`muted` in small text); a click
/// with a modifier key opens it as the browser would. The click stays with the link, so a link in a row that opens
/// something else when clicked opens only its own.
export function Link(props: { to: string; className?: string; title?: string; muted?: boolean; children: ReactNode }) {
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    e.stopPropagation();
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    navigate(props.to);
  };
  return createElement(
    PrimerLink,
    { href: props.to, className: props.className, title: props.title, muted: props.muted, onClick },
    props.children,
  );
}
