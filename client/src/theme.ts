// Light or dark: the system's, or the one the owner picked with the bar's switch, kept in this browser. The tokens
// (styles/tokens.css) follow the root element's `data-theme`, and the system's appearance where it has none.

import { useEffect, useState } from 'react';

export type Theme = 'auto' | 'light' | 'dark';

const KEY = 'wf-agents.theme';

function stored(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' ? v : 'auto';
  } catch {
    return 'auto';
  }
}

/// The theme picked, and the way to pick another.
export function useTheme(): [Theme, (t: Theme) => void] {
  const [theme, setTheme] = useState<Theme>(stored);
  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'auto') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', theme);
    try {
      if (theme === 'auto') localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, theme);
    } catch {
      // Kept only where the browser lets it be.
    }
  }, [theme]);
  return [theme, setTheme];
}
