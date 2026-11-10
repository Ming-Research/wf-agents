// Where the page is, by its address's hash: the console, the list of works, one work, or the flow. The hash keeps the
// view in the address, so that the browser's back and forward, a reload and a link all show the same view, and the
// one binary that serves the page needs no route of its own for each view.

import { useEffect, useState } from 'react';

export type Route = { view: 'console' } | { view: 'works' } | { view: 'work'; work: number } | { view: 'flow' };

/// The view an address's hash names; nothing, or a hash the page does not know, is the list of works.
export function routeOf(hash: string): Route {
  const path = hash.replace(/^#/, '').replace(/\/+$/, '');
  if (path === '/console') return { view: 'console' };
  if (path === '/flow') return { view: 'flow' };
  const work = /^\/works\/(\d+)$/.exec(path);
  if (work) return { view: 'work', work: Number(work[1]) };
  return { view: 'works' };
}

/// Whether an address's hash names a view; an empty one, or one the page does not know, names none.
export function namesView(hash: string): boolean {
  const path = hash.replace(/^#/, '').replace(/\/+$/, '');
  return path === '/works' || routeOf(hash).view !== 'works';
}

/// The hash of a view.
export function hashOf(route: Route): string {
  return route.view === 'work' ? `#/works/${route.work}` : `#/${route.view}`;
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
