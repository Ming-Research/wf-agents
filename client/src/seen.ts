// Whether the page is seen: not in a tab or a window that is hidden. The reads the page makes for its parts wait while
// it is not (works/source.ts), since no one looks at what they would bring; the notifications of what waits on the
// owner are the one thing that goes on, and ask whether the owner looks at it, as a focus is also asked (notify.ts).

import { useSyncExternalStore } from 'react';

/// Whether the page is seen now. Where there is no page to look at, as in a test drawn without a browser, it is.
export const pageSeen = (): boolean => typeof document === 'undefined' || document.visibilityState !== 'hidden';

function onVisibility(tell: () => void): () => void {
  document.addEventListener('visibilitychange', tell);
  return () => document.removeEventListener('visibilitychange', tell);
}

/// Calls `on` each time the page turns from seen to hidden, with false, and from hidden to seen, with true; gives what
/// stops it.
export function watchSeen(on: (seen: boolean) => void): () => void {
  if (typeof document === 'undefined') return () => undefined;
  let was = pageSeen();
  return onVisibility(() => {
    const now = pageSeen();
    if (now === was) return;
    was = now;
    on(now);
  });
}

/// Whether the page is seen, for a part that draws by it.
export function usePageSeen(): boolean {
  return useSyncExternalStore(
    (tell) => (typeof document === 'undefined' ? () => undefined : onVisibility(tell)),
    pageSeen,
  );
}
