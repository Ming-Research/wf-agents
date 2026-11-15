// The browser's notifications of the questions to the owner that hold a work, and of nothing else (principle
// one-count-of-what-waits). A work that holds more of them than when the items were read before is told of once, by
// its item and its latest question, and a click on it opens the item in the console, where the question is answered;
// while any is left, the owner is told again each hour how many there are. A notification that takes the place of one
// before it alerts again. The page asks the browser for leave once, at the owner's first click on it as browsers want,
// and never again whatever the answer; it tells nothing while the owner looks at the page, and nothing at all, nor
// asks, on a page that is not the engine's own (`on`).

import { useEffect, useRef } from 'react';

import { hashOf } from './route';
import type { Item, Items, Now } from './works-api';

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

/// The questions to the owner that hold a work now, in the order the work waits for them: a question its cell asked,
/// by its name, or one put to the owner as a call, by its words.
export function questionsHolding(now: Now): string[] {
  if (now.kind !== 'waits') return [];
  return now.on.flatMap((w) => {
    if (w.kind === 'answer') return [w.question];
    return w.kind === 'call' && w.what.kind === 'question' && w.what.who === 'owner' ? [w.what.question] : [];
  });
}

/// How many questions to the owner hold each work, by the work's number, for those that hold any.
export function heldByWork(items: Item[]): Map<number, number> {
  const out = new Map<number, number>();
  for (const i of items) {
    const n = questionsHolding(i.work.now).length;
    if (n > 0) out.set(i.work.number, n);
  }
  return out;
}

/// The items whose works hold more questions to the owner than they did before; none at the first reading (`before`
/// none), when every question is old. A question answered among others is no new one.
export function newlyHeld(before: Map<number, number> | null, items: Item[]): Item[] {
  if (before === null) return [];
  return items.filter((i) => questionsHolding(i.work.now).length > (before.get(i.work.number) ?? 0));
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
  useEffect(() => (on ? askOnce() : undefined), [on]);
  useEffect(() => {
    if (!on || items === null) return;
    const fresh = newlyHeld(before.current, items.items);
    before.current = heldByWork(items.items);
    left.current = [...before.current.values()].reduce((a, b) => a + b, 0);
    if (fresh.length === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
    for (const i of fresh) {
      const questions = questionsHolding(i.work.now);
      tell(
        `Waits for you: #${i.number} ${i.title}`,
        questions[questions.length - 1],
        `wf-agents-item-${i.number}`,
        hashOf({ view: 'console', item: i.number }),
      );
    }
  }, [items, on]);
  useEffect(() => {
    if (!on) return;
    const timer = setInterval(() => {
      const n = left.current;
      if (n === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
      tell(`${n} ${n === 1 ? 'question waits' : 'questions wait'} for you`, undefined, 'wf-agents-questions', '#/console');
    }, AGAIN_MS);
    return () => clearInterval(timer);
  }, [on]);
}
