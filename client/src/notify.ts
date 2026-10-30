// The browser's notifications of what newly waits for the owner, until the owner has another way to be told: each
// conversation where something more waits than when the list was read before is told of once, by its name, and a click
// on it opens it. The page asks the browser for leave once, at the owner's first click on it as browsers want, and never
// again whatever the answer; it tells nothing while the owner looks at the page, whose bar counts what waits already.

import { useEffect, useRef } from 'react';

import type { Conversation } from './api';
import { conversationTitle, newlyWaiting, waitingByKey } from './conversation';
import { href, navigate } from './route';
import { t, tn } from './text';

/// Where this browser keeps that the page asked for leave.
const ASKED = 'wf-agents.notifications-asked';

/// Whether the browser has notifications at all.
const can = (): boolean => typeof window !== 'undefined' && 'Notification' in window;

/// Asks the browser once for leave to notify, at the owner's first click on the page; nothing when it was asked before,
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

/// Tells the owner, by the browser's notifications, of each conversation where something newly waits, as the list of
/// conversations is read again; the first reading tells of nothing, since everything in it is old.
export function useNotifications(list: Conversation[] | undefined): void {
  const before = useRef<Map<string, number> | null>(null);
  useEffect(() => askOnce(), []);
  useEffect(() => {
    if (list === undefined) return;
    const fresh = newlyWaiting(before.current, list);
    before.current = waitingByKey(list);
    if (fresh.length === 0 || !can() || Notification.permission !== 'granted' || looking()) return;
    for (const c of fresh) {
      const n = new Notification(t('notify_title', { what: conversationTitle(c) }), {
        body: tn('notify_body', c.waiting),
        tag: c.key,
      });
      n.onclick = () => {
        window.focus();
        navigate(href({ page: 'messages', open: c.key }));
        n.close();
      };
    }
  }, [list]);
}
