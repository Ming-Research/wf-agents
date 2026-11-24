// Where the page is, by its address's hash: the console, with one of its items selected or none, the list of works,
// one work with one of its runs chosen, and one round of it, or none, the flow, the observations, the system over a
// span of time, or the settings. The hash keeps the view in the address, so that the browser's back and forward, a
// reload and a link all show the same view, and the one binary that serves the page needs no route of its own for each
// view.

import { useEffect, useState } from 'react';

import { CHOICE_KEYS, keptChoice } from './kept';

/// The views that are one page each, in the order of the bar's tabs.
export const PLAIN = ['console', 'works', 'flow', 'observations', 'system', 'settings'] as const;
export type Plain = (typeof PLAIN)[number];

/// The span of time the system's view counts over: the week under way, the four weeks to this one's end, or all time.
export const PERIODS = ['week', '4w', 'all'] as const;
export type Period = (typeof PERIODS)[number];

/// A plain view; the console with an item selected; the system's over the span its address names; or one
/// work, with one of its runs chosen where the address names it, and one round of that run, which a round is only
/// named with its run.
export type Route =
  | { view: Plain }
  | { view: 'console'; item: number; wait?: string }
  | { view: 'system'; period: Period }
  | { view: 'work'; work: number; run?: number; round?: number };

const pathOf = (hash: string): string => hash.replace(/^#/, '').replace(/\/+$/, '');

/// A part of an address as it was before it was encoded, or none where it is not one encoded.
function decoded(part: string): string | null {
  try {
    return decodeURIComponent(part);
  } catch {
    return null;
  }
}

/// The view an address's hash names, if it names one.
function named(hash: string): Route | null {
  const path = pathOf(hash);
  const plain = PLAIN.find((v) => path === `/${v}`);
  if (plain) return { view: plain };
  const work = /^\/works\/(\d+)(?:\/runs\/(\d+)(?:\/rounds\/(\d+))?)?$/.exec(path);
  if (work) {
    if (work[2] === undefined) return { view: 'work', work: Number(work[1]) };
    return work[3] === undefined
      ? { view: 'work', work: Number(work[1]), run: Number(work[2]) }
      : { view: 'work', work: Number(work[1]), run: Number(work[2]), round: Number(work[3]) };
  }
  // The key of what waits holds the name of a question, which may hold any character: the address carries it encoded.
  const item = /^\/console\/items\/(\d+)(?:\/waits\/([^/]+))?$/.exec(path);
  if (item) {
    if (item[2] === undefined) return { view: 'console', item: Number(item[1]) };
    const wait = decoded(item[2]);
    return wait === null ? null : { view: 'console', item: Number(item[1]), wait };
  }
  const period = PERIODS.find((p) => path === `/system/${p}`);
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
    if (route.run === undefined) return `#/works/${route.work}`;
    const run = `#/works/${route.work}/runs/${route.run}`;
    return route.round === undefined ? run : `${run}/rounds/${route.round}`;
  }
  if ('item' in route) {
    return route.wait === undefined
      ? `#/console/items/${route.item}`
      : `#/console/items/${route.item}/waits/${encodeURIComponent(route.wait)}`;
  }
  if ('period' in route) return `#/system/${route.period}`;
  return `#/${route.view}`;
}

/// The span the system's view counts over: the one the address names, else the one the owner last picked, kept in this
/// browser, else the week under way.
export function periodOf(route: Route): Period {
  return 'period' in route ? route.period : keptChoice(CHOICE_KEYS.systemPeriod, 'week', PERIODS);
}

/// The tab of the bar a view belongs to: a work belongs to the works, and the system over any span to the system.
export function tabOf(route: Route): string {
  return route.view === 'work' ? '#/works' : `#/${route.view}`;
}

/// Whether the page itself brought the owner to the view it shows: true once the address has changed while the page
/// has been open, which is when going back lands on another of its views and not on whatever stood before the page.
let brought = false;

/// Whether the address the page is now changing is one it replaces itself (`replaceHash`), which brings the owner to
/// no other view.
let replacing = false;

/// Replaces the address with `hash`, adding no entry to the browser's history, where it is not the address already:
/// the one way the page replaces its address, as when what is chosen in a view is written to it. An address replaced so
/// does not count as one the page brought the owner to, so that going back from a page opened on a view does not leave
/// the page.
export function replaceHash(hash: string) {
  if (window.location.hash === hash) return;
  replacing = true;
  window.location.replace(hash);
}

/// The view the address names now, followed as it changes.
export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(() => routeOf(window.location.hash));
  useEffect(() => {
    const changed = () => {
      if (replacing) replacing = false;
      else brought = true;
      setRoute(routeOf(window.location.hash));
    };
    window.addEventListener('hashchange', changed);
    return () => window.removeEventListener('hashchange', changed);
  }, []);
  return route;
}

/// Goes back where the owner came from, where the page itself brought them to the view they are on, so that a work
/// opened from the console leads back to the console and one opened from the works to the works; and to `to`, the
/// view the way back names, where the page was opened on this view and there is nothing of it to go back to.
export function goBack(to: string) {
  if (brought) window.history.back();
  else window.location.hash = to;
}
