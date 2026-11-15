// The browser's notifications of what waits on the owner, as `waitsOf` says, and of nothing else (principle
// one-count-of-what-waits). A work that waits for more than when the items were read before is told of once, by its
// item and its latest wait, and a click on it opens the console on the item at the card of that wait; while any is
// left, the owner is told again each hour how many there are, and a click opens the first of them. A notification that takes the place of one
// before it alerts again. The page asks the browser for leave once, at the owner's first click on it as browsers want,
// and never again whatever the answer; it tells nothing while the owner looks at the page, and nothing at all, nor
// asks, on a page that is not the engine's own (`on`).

import { useEffect, useRef } from 'react';

import { hashOf } from './route';
import type { Item, Items } from './works-api';
import { type Wait, targetOf, waitsOf } from './works/waiting';

/// Where this browser keeps that the page asked for leave.
const ASKED = 'wf-agents.notifications-asked';

/// How often the owner is told again of the questions that are left, while any is.
export const AGAIN_MS = 60 * 60 * 1000;

/// Whether the browser has notifications at all.
const can = (): boolean => typeof window !== 'undefined' && 'Notification' in window;

/// Asks the browser once for leave to notify, at the owner's first click on the page; nothing where it was asked before
/// in this browser, or answered. Gives what stops waiting for the click.
function askOnce(): (() => void) | undefined {
  if (!can() || Notification.permission !== 'default') return undefined;
  try {
    if (window.localStorage.getItem(ASKED) !== null) return undefined;
  } catch {
    // A browser that keeps nothing is asked each time the page opens, at most once.
  }
  const ask = () => {
    try {
      window.localStorage.setItem(ASKED, String(Date.now()));
    } catch {
      // Kept only where the browser lets it be.
    }
    Notification.requestPermission().catch(() => {
      // Refused or not asked: the page tells nothing.
    });
  };
  window.addEventListener('pointerdown', ask, { once: true });
  return () => window.removeEventListener('pointerdown', ask);
}

/// Whether the owner looks at the page now.
const looking = (): boolean => document.visibilityState === 'visible' && document.hasFocus();

/// How many things wait for the owner in each work, by the work's number, for those that wait for any; what counts is
/// `waitsOf`, the one definition the bar's count and the cards share.
export function heldByWork(items: Item[]): Map<number, number> {
  const out = new Map<number, number>();
  for (const i of items) {
    const n = waitsOf(i.work.now).length;
    if (n > 0) out.set(i.work.number, n);
  }
  return out;
}

/// The items whose works wait for the owner for more than they did before; none at the first reading (`before` none),
/// when everything is old. Something answered among others is no new one.
export function newlyHeld(before: Map<number, number> | null, items: Item[]): Item[] {
  if (before === null) return [];
  return items.filter((i) => waitsOf(i.work.now).length > (before.get(i.work.number) ?? 0));
}

/// What a notification of an item opens: the item in the console, at the card of its latest wait.
export function openingOf(item: Item): { wait: Wait; opens: string } | null {
  const waits = waitsOf(item.work.now);
  const wait = waits[waits.length - 1];
  return wait === undefined ? null : { wait, opens: hashOf(targetOf(item, wait)) };
}

/// What the hourly notification opens: the first item that waits, at the card of its first wait.
export function firstOpening(items: Item[]): string | null {
  for (const i of items) {
    const wait = waitsOf(i.work.now)[0];
    if (wait !== undefined) return hashOf(targetOf(i, wait));
  }
  return null;
}

/// A notification that takes the place of one with its tag, and alerts again as it does.
function tell(title: string, body: string | undefined, tag: string, opens: string) {
  const options: NotificationOptions & { renotify: boolean } = { body, tag, renotify: true };
  const n = new Notification(title, options);
  n.onclick = () => {
    window.focus();
    window.location.hash = opens;
    n.close();
  };
}

/// Tells the owner, by the browser's notifications, of each work that newly holds a question to them, as the items
/// are read again, and again each hour how many questions are left, while any is; on a page that is not the engine's
/// own (`on` false), nothing.
export function useNotifications(items: Items | null, on: boolean): void {
  const before = useRef<Map<number, number> | null>(null);
  const left = useRef(0);
  const first = useRef<string | null>(null);
  useEffect(() => (on ? askOnce() : undefined), [on]);
  useEffect(() => {
    if (!on || items === null) return;
    const fresh = newlyHeld(before.current, items.items);
    before.current = heldByWork(items.items);
    left.current = [...before.current.values()].reduce((a, b) => a + b, 0);
    first.current = firstOpening(items.items);
    if (fresh.length === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
    for (const i of fresh) {
      const opening = openingOf(i);
      if (opening === null) continue;
      tell(`Waits for you: #${i.number} ${i.title}`, opening.wait.words, `wf-agents-item-${i.number}`, opening.opens);
    }
  }, [items, on]);
  useEffect(() => {
    if (!on) return;
    const timer = setInterval(() => {
      const n = left.current;
      if (n === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
      tell(`${n} ${n === 1 ? 'thing waits' : 'things wait'} for you`, undefined, 'wf-agents-questions', first.current ?? '#/console');
    }, AGAIN_MS);
    return () => clearInterval(timer);
  }, [on]);
}
