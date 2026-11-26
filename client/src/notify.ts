// The browser's notifications of what waits on the owner, as the engine's one list of it says, and of nothing else
// (principle one-count-of-what-waits). A work that waits for more than when the items were read before is told of
// once, by its item, with why it waits for its latest wait, what it asks for a question, and a click on it opens the
// console on the item at the card of that wait; what the whole installation waits on the owner for, which no work does,
// is told of once each time it begins, and a click on it opens the console, where the bar holds the control that ends
// it; while any is left, the owner is told again each hour how many there
// are, in the bar's words, and a click opens the first of them. A notification that takes the place of one before it
// alerts again. The page asks the browser for leave once, at the owner's first click on it as browsers want,
// and never again whatever the answer; it tells nothing while the owner looks at the page, and nothing at all, nor
// asks, on a page that is not the engine's own (`on`).

import { useEffect, useRef } from 'react';

import { hashOf } from './route';
import type { InstallationWait, Item, Items } from './works-api';
import { type Wait, installationWaits, targetOf, waitsOf } from './works/waiting';
import { STATE, firstLine, itemName, plain, sentence, stateOf, waitCount } from './works/words';

/// What a notification of a wait says: the cell the work waits at, as a line names it, then the wait's reason, the
/// first line of what a question asks, as a sentence, its names bare.
export function bodyOf(item: Item, wait: Wait): string {
  const why = plain(sentence(firstLine(wait.why)));
  const { cell } = stateOf(item.work);
  return cell === null ? why : `${cell} · ${why}`;
}

/// Where this browser keeps that the page asked for leave.
const ASKED = 'wf-agents.notifications-asked';

/// How often the owner is told again of what is left, while anything is.
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

/// How many things wait for the owner in each work, by the work's number, for those that wait for any: the engine's
/// list, which the bar's count and the cards read.
export function heldByWork(items: Item[]): Map<number, number> {
  const out = new Map<number, number>();
  for (const i of items) {
    const n = waitsOf(i.work).length;
    if (n > 0) out.set(i.work.number, n);
  }
  return out;
}

/// The items whose works wait for the owner for more than they did before; none at the first reading (`before` none),
/// when everything is old. Something answered among others is no new one.
export function newlyHeld(before: Map<number, number> | null, items: Item[]): Item[] {
  if (before === null) return [];
  return items.filter((i) => waitsOf(i.work).length > (before.get(i.work.number) ?? 0));
}

/// What names a wait of the installation for as long as it stands: its key and since when it has stood, so that one that
/// ends and begins again is told of again.
export const waitId = (w: Pick<InstallationWait, 'key' | 'since'>): string => `${w.key}@${w.since}`;

/// The waits of the installation that are new since `known` was read; none at the first reading (`known` none), when
/// everything is old.
export function newInstallationWaits(known: ReadonlySet<string> | null, waits: InstallationWait[]): InstallationWait[] {
  if (known === null) return [];
  return waits.filter((w) => !known.has(waitId(w)));
}

/// What a notification of an item opens: the item in the console, at the card of its latest wait.
export function openingOf(item: Item): { wait: Wait; opens: string } | null {
  const waits = waitsOf(item.work);
  const wait = waits[waits.length - 1];
  return wait === undefined ? null : { wait, opens: hashOf(targetOf(item, wait)) };
}

/// What the hourly notification opens: the first item that waits, at the card of its first wait.
export function firstOpening(items: Item[]): string | null {
  for (const i of items) {
    const wait = waitsOf(i.work)[0];
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

/// Tells the owner, by the browser's notifications, of each work that newly waits on them, as the items are read
/// again, and again each hour how many things are left, while any is; on a page that is not the engine's own (`on`
/// false), nothing.
export function useNotifications(items: Items | null, on: boolean): void {
  const before = useRef<Map<number, number> | null>(null);
  const known = useRef<Set<string> | null>(null);
  const left = useRef(0);
  const first = useRef<string | null>(null);
  useEffect(() => (on ? askOnce() : undefined), [on]);
  useEffect(() => {
    if (!on || items === null) return;
    const fresh = newlyHeld(before.current, items.items);
    const everything = installationWaits(items);
    const freshEverything = newInstallationWaits(known.current, everything);
    before.current = heldByWork(items.items);
    known.current = new Set(everything.map(waitId));
    left.current = [...before.current.values()].reduce((a, b) => a + b, 0) + everything.length;
    first.current = firstOpening(items.items);
    if (
      (fresh.length === 0 && freshEverything.length === 0) ||
      !can() ||
      Notification.permission !== 'granted' ||
      looking()
    )
      return;
    for (const w of freshEverything) {
      tell(`${STATE.owner}: ${plain(sentence(firstLine(w.why)))}`, undefined, `wf-agents-${w.key}`, '#/console');
    }
    for (const i of fresh) {
      const opening = openingOf(i);
      if (opening === null) continue;
      tell(
        `${STATE.owner}: ${itemName(i.number)} ${i.title}`,
        bodyOf(i, opening.wait),
        `wf-agents-item-${i.number}`,
        opening.opens,
      );
    }
  }, [items, on]);
  useEffect(() => {
    if (!on) return;
    const timer = setInterval(() => {
      const n = left.current;
      if (n === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
      tell(waitCount(n), undefined, 'wf-agents-waits', first.current ?? '#/console');
    }, AGAIN_MS);
    return () => clearInterval(timer);
  }, [on]);
}
