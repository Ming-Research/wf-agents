// What the server the page came from gives. `wf-agents flow page` gives the flow alone: it has no items and no works,
// and answers that it has no such path. The page asks once, so that the console and the works can say so plainly and
// offer the flow, where they would otherwise show a failed read.

import { useEffect, useState } from 'react';

import { namesView } from './route';
import { INTERFACE, sourceOf } from './works/source';

export type Serves = 'all' | 'flow';

type Ask = (path: string) => Promise<{ status: number }>;

/// Asks for the path's head alone: the answer's status is all that is read, and no list is sent.
const askInterface: Ask = (path) => fetch(path, { method: 'HEAD' });

/// Whether the server has the items and the works. A page answered from the fixtures has them and asks nothing. A
/// server that cannot be asked is taken to have them: the views then say what failed.
export async function askServes(search: string, ask: Ask = askInterface): Promise<Serves> {
  if (sourceOf(search) !== INTERFACE) return 'all';
  try {
    return (await ask('/api/v1/items')).status === 404 ? 'flow' : 'all';
  } catch {
    return 'all';
  }
}

/// Whether the page opens on the flow: the server gives the flow alone, and the address names no view.
export function opensOnFlow(serves: Serves | null, hash: string): boolean {
  return serves === 'flow' && !namesView(hash);
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
