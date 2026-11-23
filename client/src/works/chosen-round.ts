// The round the work page's address names, which the Step draws open and lit: given by the work's page (`WorkScreen`)
// to the round parts of the Step it holds, while the run chosen is the address's run, and none otherwise.

import { createContext } from 'react';

/// A round named by the address: its run and its number among the run's rounds, and whether the Step has been scrolled
/// to it since the address came to name it, kept in one place so that drawing the Step again does not scroll again.
export interface NamedRound {
  run: number;
  round: number;
  scrolled: { current: boolean };
}

export const ChosenRound = createContext<NamedRound | null>(null);
