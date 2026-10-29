// What the agents spent by the line of their session and the model they ran on, apart from how the page draws it: a
// table of lines by models, the costliest of each first, with each model's total and the whole. The advisor runs one
// tier above the engineer, and its spend on the top tier is what the owner watches.

import type { LineSpend } from './api';

export type SpendRow = { key: string; byModel: Record<string, number>; total: number };

export type SpendTable = {
  /// The models, the costliest first.
  models: string[];
  /// Each line, the costliest first.
  lines: SpendRow[];
  byModel: Record<string, number>;
  total: number;
};

const add = (to: Record<string, number>, key: string, n: number) => {
  to[key] = (to[key] ?? 0) + n;
};

/// The spend by line and model, from what each line spent on each model.
export function spendTable(rows: LineSpend[]): SpendTable {
  const byModel: Record<string, number> = {};
  const lines = new Map<string, SpendRow>();
  for (const r of rows) {
    add(byModel, r.model, r.cost_usd);
    const line = lines.get(r.line) ?? { key: r.line, byModel: {}, total: 0 };
    add(line.byModel, r.model, r.cost_usd);
    line.total += r.cost_usd;
    lines.set(r.line, line);
  }
  const models = Object.keys(byModel).sort((a, b) => byModel[b] - byModel[a]);
  return {
    models,
    lines: [...lines.values()].sort((a, b) => b.total - a.total),
    byModel,
    total: Object.values(byModel).reduce((a, b) => a + b, 0),
  };
}
