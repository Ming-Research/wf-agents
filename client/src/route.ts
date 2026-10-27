// The page's addresses: each view has a path, so it can be reloaded, linked and gone back to.

import { createElement, useMemo, useSyncExternalStore, type MouseEvent, type ReactNode } from 'react';

export type Route =
  | { page: 'works' }
  | { page: 'work'; id: number; round: number | null; cell: string | null }
  | { page: 'items' }
  | { page: 'item'; id: number }
  | { page: 'messages' }
  | {
      page: 'flow';
      /// The stored version shown; with none, the one new works start on.
      version: number | null;
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

export function parse(pathname: string, search: string): Route {
  const q = new URLSearchParams(search);
  const parts = pathname.split('/').filter((p) => p !== '');
  const [first, second] = parts;
  if (parts.length === 0 || (first === 'works' && parts.length === 1)) return { page: 'works' };
  if (first === 'works' && parts.length === 2) {
    const id = num(second);
    if (id !== null) return { page: 'work', id, round: num(q.get('round')), cell: q.get('cell') };
  }
  if (first === 'items' && parts.length === 1) return { page: 'items' };
  if (first === 'messages' && parts.length === 1) return { page: 'messages' };
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
      return `/works/${r.id}${query([['round', r.round], ['cell', r.cell]])}`;
    case 'items':
      return '/items';
    case 'item':
      return `/items/${r.id}`;
    case 'messages':
      return '/messages';
    case 'flow':
      return `/flow${r.version === null ? '' : `/${r.version}`}${query([
        ['project', r.project],
        ['ref', r.ref],
        ['from', r.from],
        ['cell', r.cell],
      ])}`;
    case 'missing':
      return '/';
  }
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

/// A link that moves within the page without loading it again; a click with a modifier key opens it as the browser
/// would.
export function Link(props: { to: string; className?: string; title?: string; children: ReactNode }) {
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    navigate(props.to);
  };
  return createElement('a', { href: props.to, className: props.className, title: props.title, onClick }, props.children);
}
