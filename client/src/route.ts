// Where the page is, by its address's hash: the console, the list of works, one work with one of its runs chosen or
// none, the flow, the observations, the system over a span of time, or the settings. The hash keeps the view in the
// address, so that the browser's back and forward, a reload and a link all show the same view, and the one binary that
// serves the page needs no route of its own for each view.

import { useEffect, useState } from 'react';

/// The views that are one page each, in the order of the bar's tabs.
export const PLAIN = ['console', 'works', 'flow', 'observations', 'system', 'settings'] as const;
export type Plain = (typeof PLAIN)[number];

/// The span of time the system's view counts over: the week under way, the four weeks to this one's end, or all time.
export const PERIODS = ['week', '4w', 'all'] as const;
export type Period = (typeof PERIODS)[number];

/// A plain view; the system's over a span other than the week under way; or one work, with one of its runs chosen
/// where the address names it.
export type Route = { view: Plain } | { view: 'system'; period: Period } | { view: 'work'; work: number; run?: number };

const pathOf = (hash: string): string => hash.replace(/^#/, '').replace(/\/+$/, '');

/// The view an address's hash names, if it names one.
function named(hash: string): Route | null {
  const path = pathOf(hash);
  const plain = PLAIN.find((v) => path === `/${v}`);
  if (plain) return { view: plain };
  const work = /^\/works\/(\d+)(?:\/runs\/(\d+))?$/.exec(path);
  if (work) {
    return work[2] === undefined
      ? { view: 'work', work: Number(work[1]) }
      : { view: 'work', work: Number(work[1]), run: Number(work[2]) };
  }
  const period = PERIODS.find((p) => p !== 'week' && path === `/system/${p}`);
  return period ? { view: 'system', period } : null;
}

/// The view an address's hash names; nothing, or a hash the page does not know, is the console, which the page is
/// centred on.
export function routeOf(hash: string): Route {
  return named(hash) ?? { view: 'console' };
}

/// Whether an address's hash names a view; an empty one, or one the page does not know, names none.
export function namesView(hash: string): boolean {
  return named(hash) !== null;
}

/// The hash of a view.
export function hashOf(route: Route): string {
  if (route.view === 'work') {
    return route.run === undefined ? `#/works/${route.work}` : `#/works/${route.work}/runs/${route.run}`;
  }
  if ('period' in route && route.period !== 'week') return `#/system/${route.period}`;
  return `#/${route.view}`;
}

/// The span the system's view counts over: the week under way unless the address names another.
export function periodOf(route: Route): Period {
  return 'period' in route ? route.period : 'week';
}

/// The tab of the bar a view belongs to: a work belongs to the works, and the system over any span to the system.
export function tabOf(route: Route): string {
  return route.view === 'work' ? '#/works' : `#/${route.view}`;
}

/// The view the address names now, followed as it changes.
export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => routeOf(window.location.hash));
  useEffect(() => {
    const changed = () => setRoute(routeOf(window.location.hash));
    window.addEventListener('hashchange', changed);
    return () => window.removeEventListener('hashchange', changed);
  }, []);
  return route;
}
