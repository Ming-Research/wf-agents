// What the agents spent across every work, apart from how the page draws it: by model and line, and by day, with the
// advisor's part of each model's spend apart, since the advisor runs one tier above the engineer and its spend on the
// top tier is what the owner watches.

import type { Spend } from './api';

export type SpendRow = { key: string; byModel: Record<string, number>; total: number };

export type SpendDay = SpendRow & { advisor: Record<string, number> };

export type SpendTable = {
  /// The models, the costliest first.
  models: string[];
  /// Each line, the costliest first.
  lines: SpendRow[];
  /// Each day, the latest first.
  days: SpendDay[];
  byModel: Record<string, number>;
  total: number;
};

const add = (to: Record<string, number>, key: string, n: number) => {
  to[key] = (to[key] ?? 0) + n;
};

/// The spend across every work, from what each day, model and line spent.
export function spendTable(rows: Spend[]): SpendTable {
  const byModel: Record<string, number> = {};
  const lines = new Map<string, SpendRow>();
  const days = new Map<string, SpendDay>();
  for (const r of rows) {
    add(byModel, r.model, r.cost_usd);
    const line = lines.get(r.line) ?? { key: r.line, byModel: {}, total: 0 };
    add(line.byModel, r.model, r.cost_usd);
    line.total += r.cost_usd;
    lines.set(r.line, line);
    const day = days.get(r.day) ?? { key: r.day, byModel: {}, advisor: {}, total: 0 };
    add(day.byModel, r.model, r.cost_usd);
    if (r.line === 'advisor') add(day.advisor, r.model, r.cost_usd);
    day.total += r.cost_usd;
    days.set(r.day, day);
  }
  const models = Object.keys(byModel).sort((a, b) => byModel[b] - byModel[a]);
  return {
    models,
    lines: [...lines.values()].sort((a, b) => b.total - a.total),
    days: [...days.values()].sort((a, b) => (a.key < b.key ? 1 : -1)),
    byModel,
    total: Object.values(byModel).reduce((a, b) => a + b, 0),
  };
}
