// The console alone, for looking at it before the page's shell holds it (`console.html`, served by `npm run dev`;
// `console.html?fixtures` draws the made-up items). Not built into the page: the shell's entry is `main.tsx`.

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import '../styles/tokens.css';
import '../styles/parts.css';
import { useTheme } from '../theme';
import { ConsolePage } from './index';

/// The console with the theme kept in this browser, which the bar's switch would set.
function Alone() {
  useTheme();
  return <ConsolePage />;
}

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <Alone />
    </StrictMode>,
  );
}
