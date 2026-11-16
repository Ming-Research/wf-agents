// The page's entry: its tokens and parts, the chart's styles and the flowchart's, the works page's, the observations',
// and the page.

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { startAccent } from './accent';
import { App } from './App';
import './styles/tokens.css';
import './styles/parts.css';
import './styles/chart.css';
import './styles/graph.css';
import './styles/works.css';
import './styles/observations.css';
import './styles/system.css';

// The accent kept in this browser, set before the page is drawn, so that it is drawn in it from the first.
startAccent();

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
