// MOCK. What the bar shows of the engine that the engine does not give or do yet: the pause of every work, keeping
// waiting sessions warm, and how many slots the installation has. Each is kept in this browser only and changes
// nothing in the engine. When the engine has them, this file is replaced by what it gives, and the bar's part
// (Engine.tsx) reads that instead.

import { useCallback, useState } from 'react';

const KEY = 'wf-agents.mock.engine';

export interface MockEngine {
  /// Every work is paused.
  paused: boolean;
  /// Waiting sessions are kept warm.
  keepWarm: boolean;
  /// The slots of the installation.
  slots: number;
}

const FIRST: MockEngine = { paused: false, keepWarm: true, slots: 2 };

function stored(): MockEngine {
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<MockEngine> | null;
    return { ...FIRST, ...kept };
  } catch {
    return FIRST;
  }
}

/// The mocked state, and the way to change a part of it.
export function useMockEngine(): [MockEngine, (change: Partial<MockEngine>) => void] {
  const [state, setState] = useState<MockEngine>(stored);
  const change = useCallback((part: Partial<MockEngine>) => {
    setState((was) => {
      const now = { ...was, ...part };
      try {
        localStorage.setItem(KEY, JSON.stringify(now));
      } catch {
        // Kept only where the browser lets it be.
      }
      return now;
    });
  }, []);
  return [state, change];
}
