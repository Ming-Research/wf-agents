// The page's entry: Primer's tokens and themes, the page's own styles after them, and the page inside Primer's theme,
// which follows the system's light or dark appearance as the root element in index.html says. Brought over from the
// page of the engine on `main`.

import '@primer/primitives/dist/css/primitives.css';
import '@primer/primitives/dist/css/functional/themes/light.css';
import '@primer/primitives/dist/css/functional/themes/dark.css';
import { BaseStyles } from '@primer/react';
import { ThemeProvider } from '@primer/react/next';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import './styles.css';

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <ThemeProvider colorMode="auto">
        <BaseStyles>
          <App />
        </BaseStyles>
      </ThemeProvider>
    </StrictMode>,
  );
}
