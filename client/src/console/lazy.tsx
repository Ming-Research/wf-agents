// The console as the page's shell mounts it: read only when it is first shown, inside a `Suspense`, so that its drawing
// library (PixiJS, the largest part of the page) is not in what every view loads first.

import { lazy } from 'react';

export const LazyConsolePage = lazy(() => import('./index').then((m) => ({ default: m.ConsolePage })));
