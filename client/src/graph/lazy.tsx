// The flowchart as the flow view mounts it: read only when it is first shown, inside a `Suspense`, so that its layout
// library (elkjs) is not in what the page loads first.

import { lazy } from 'react';

export const LazyFlowChart = lazy(() => import('./FlowChart'));
