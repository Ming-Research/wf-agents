// The browser's notifications of what newly waits for the owner: what the bar counts (`waitsForOwner`, Engine.tsx) and
// nothing else. Each work that comes to wait for the owner for something new, a question, a stopped work, a fault, is
// told of once, by its item and what it waits for, and a click on it opens the work; while anything is left waiting,
// the owner is told again each hour how many works wait. The page asks the browser for leave once, at the owner's first
// click on it as browsers want, and never again whatever the answer; it tells nothing while the owner looks at the page,
// whose bar counts what waits already. Brought over from the page on `old-main` (`client/src/notify.ts`), which told of
// conversations.

import { useEffect, useRef } from 'react';

import { whoGives } from './console/items';
import { waitsForOwner } from './Engine';
import type { Item, Items, Now, Waited } from './works-api';
import { nowWords } from './works/words';

/// Where this browser keeps that the page asked for leave.
const ASKED = 'wf-agents.notifications-asked';

/// How often the owner is told again that works wait, while any does.
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

/// What of a wait is the owner's to give, by what tells one apart from another.
function waitedKey(w: Waited): string {
  switch (w.kind) {
    case 'call':
      return `call ${w.call}`;
    case 'precondition':
      return `precondition ${w.test}`;
    case 'agents':
      return `agents ${w.outage.failure}`;
    default:
      return w.kind;
  }
}

/// What a work waits for the owner for, as a key that differs whenever it waits for something new; none where it waits
/// for nothing of the owner's.
export function waitingKey(now: Now): string | null {
  if (!waitsForOwner(now)) return null;
  switch (now.kind) {
    case 'fault':
      return `fault ${now.words}`;
    case 'paused':
      return `paused ${now.why ?? ''}`;
    case 'waits':
      return `waits ${now.at}: ${now.on.filter((w) => whoGives(w) === 'owner').map(waitedKey).join(', ')}`;
    default:
      return now.kind;
  }
}

/// What each work waits for the owner for, by the work's number.
export function waitingByWork(items: Item[]): Map<number, string> {
  const out = new Map<number, string>();
  for (const i of items) {
    const key = waitingKey(i.work.now);
    if (key !== null) out.set(i.work.number, key);
  }
  return out;
}

/// The items whose works wait for the owner for something they did not wait for before; none at the first reading
/// (`before` none), when everything that waits is old.
export function newlyWaiting(before: Map<number, string> | null, items: Item[]): Item[] {
  if (before === null) return [];
  return items.filter((i) => {
    const key = waitingKey(i.work.now);
    return key !== null && before.get(i.work.number) !== key;
  });
}

/// Tells the owner, by the browser's notifications, of each work that newly waits for them, as the items are read
/// again, and again each hour how many wait, while any does.
export function useNotifications(items: Items | null): void {
  const before = useRef<Map<number, string> | null>(null);
  const waiting = useRef(0);
  useEffect(() => askOnce(), []);
  useEffect(() => {
    if (items === null) return;
    const fresh = newlyWaiting(before.current, items.items);
    before.current = waitingByWork(items.items);
    waiting.current = before.current.size;
    if (fresh.length === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
    for (const i of fresh) {
      const n = new Notification(`Waits for you: #${i.number} ${i.title}`, {
        body: nowWords(i.work.now),
        tag: `wf-agents-work-${i.work.number}`,
      });
      n.onclick = () => {
        window.focus();
        window.location.hash = `#/works/${i.work.number}`;
        n.close();
      };
    }
  }, [items]);
  useEffect(() => {
    const timer = setInterval(() => {
      const n = waiting.current;
      if (n === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
      const told = new Notification(`${n} ${n === 1 ? 'work waits' : 'works wait'} for you`, { tag: 'wf-agents-waiting' });
      told.onclick = () => {
        window.focus();
        window.location.hash = '#/console';
        told.close();
      };
    }, AGAIN_MS);
    return () => clearInterval(timer);
  }, []);
}
