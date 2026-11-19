// Light or dark: the page is dark, or the one the owner picked with the bar's switch, kept in this browser. The
// tokens (styles/tokens.css) follow the root element's `data-theme`, and the system's appearance where it has none,
// which is what the switch's `auto` leaves it to.

import { useEffect, useState } from 'react';

export type Theme = 'auto' | 'light' | 'dark';

const KEY = 'wf-agents.theme';

/// What the page is drawn in while the owner has picked nothing. `auto`, the system's, is a pick of its own and is
/// kept as one, so that it holds from one visit to the next.
const DEFAULT: Theme = 'dark';

function stored(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' || v === 'auto' ? v : DEFAULT;
  } catch {
    return DEFAULT;
  }
}

/// Draws the page in `theme`: `auto` leaves the root with no `data-theme`, where the tokens follow the system.
function draw(theme: Theme) {
  const root = document.documentElement;
  if (theme === 'auto') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', theme);
}

// The theme kept is drawn as the page's code is read, before any of it is painted, so that a page in dark does not
// flash the system's light first.
if (typeof document !== 'undefined') draw(stored());

/// The theme picked, and the way to pick another.
export function useTheme(): [Theme, (t: Theme) => void] {
  const [theme, setTheme] = useState<Theme>(stored);
  useEffect(() => {
    draw(theme);
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      // Kept only where the browser lets it be.
    }
  }, [theme]);
  return [theme, setTheme];
}
