// Where the page is, by its address's hash: the console, the list of works, one work with one of its runs chosen or
// none, the flow, the observations, the system or the settings. The hash keeps the view in the address, so that the
// browser's back and forward, a reload and a link all show the same view, and the one binary that serves the page needs
// no route of its own for each view.

import { useEffect, useState } from 'react';

/// The views that are one page each, in the order of the bar's tabs.
export const PLAIN = ['console', 'works', 'flow', 'observations', 'system', 'settings'] as const;
export type Plain = (typeof PLAIN)[number];

/// One work, with one of its runs chosen where the address names it.
export type Route = { view: Plain } | { view: 'work'; work: number; run?: number };

const pathOf = (hash: string): string => hash.replace(/^#/, '').replace(/\/+$/, '');

/// The view an address's hash names, if it names one.
function named(hash: string): Route | null {
  const path = pathOf(hash);
  const plain = PLAIN.find((v) => path === `/${v}`);
  if (plain) return { view: plain };
  const work = /^\/works\/(\d+)(?:\/runs\/(\d+))?$/.exec(path);
  if (!work) return null;
  return work[2] === undefined
    ? { view: 'work', work: Number(work[1]) }
    : { view: 'work', work: Number(work[1]), run: Number(work[2]) };
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
  if (route.view !== 'work') return `#/${route.view}`;
  return route.run === undefined ? `#/works/${route.work}` : `#/works/${route.work}/runs/${route.run}`;
}

/// The tab of the bar a view belongs to: a work belongs to the works.
export function tabOf(route: Route): string {
  return route.view === 'work' ? '#/works' : hashOf(route);
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
