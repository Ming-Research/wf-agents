// What the server the page came from gives. `wf-agents flow page` gives the flow alone: it has no items and no works,
// and answers that it has no such path. The page asks once, so that the console and the works can say so plainly and
// offer the flow, where they would otherwise show a failed read.

import { useEffect, useState } from 'react';

export type Serves = 'all' | 'flow';

type Ask = (path: string) => Promise<{ status: number }>;

const askInterface: Ask = (path) => fetch(path, { headers: { accept: 'application/json' } });

/// Whether the server has the items and the works. With `?fixtures` the page is answered from the fixtures, which
/// have them. A server that cannot be asked is taken to have them: the views then say what failed.
export async function askServes(search: string, ask: Ask = askInterface): Promise<Serves> {
  if (new URLSearchParams(search).has('fixtures')) return 'all';
  try {
    return (await ask('/api/v1/items')).status === 404 ? 'flow' : 'all';
  } catch {
    return 'all';
  }
}

/// What the server gives; nothing while it is being asked.
export function useServes(): Serves | null {
  const [serves, setServes] = useState<Serves | null>(null);
  useEffect(() => {
    let shown = true;
    void askServes(window.location.search).then((s) => {
      if (shown) setServes(s);
    });
    return () => {
      shown = false;
    };
  }, []);
  return serves;
}
