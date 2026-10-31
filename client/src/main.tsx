// The page's entry: Primer's tokens and themes, the page's own palette over them (theme.css) and its own styles after
// them, and the app inside Primer's theme, which follows the system's light or dark appearance as the root element in
// index.html says.

import '@primer/primitives/dist/css/primitives.css';
import '@primer/primitives/dist/css/functional/themes/light.css';
import '@primer/primitives/dist/css/functional/themes/dark.css';
import { BaseStyles } from '@primer/react';
import { ThemeProvider } from '@primer/react/next';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import './styles/theme.css';
import './styles/tokens.css';
import './styles/base.css';
import './styles/roles.css';
import './styles/shell.css';
import './styles/parts.css';
import './styles/cards.css';
import './styles/item.css';
import './styles/screen.css';
import './styles/panels.css';
import './styles/flow.css';
import './styles/settings.css';
import './styles/charts.css';
import './styles/works.css';
import './styles/usage.css';
import './styles/architect.css';
import './styles/items.css';
import './styles/observations.css';
import './styles/system.css';
import './styles/chart.css';
import './styles/messages.css';

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
