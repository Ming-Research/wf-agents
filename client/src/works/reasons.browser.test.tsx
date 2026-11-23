// What the Step shows of a return that was sent back, in a real browser (Chromium): a value recorded of a reason keeps
// its line breaks, as the files git lists come one to a line and the page names no field to say which value holds
// them. Only a browser lays text out, which the stand-in for it that the page's other tests draw in does not; the rule
// of `works.css` that does it is seen at work by this test alone.

import '../styles/tokens.css';
import '../styles/parts.css';
import '../styles/works.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { describe, expect, it } from 'vitest';

import { Reasons } from './StepPanel';

/// The number of lines the browser lays the value `changes` of a reason out in, drawn as the Step draws it: an inline
/// box has one rectangle for each line it takes.
function linesOf(changes: string): number {
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  const unmet = [{ key: 'uncommitted', values: [{ name: 'changes', value: changes }] }];
  flushSync(() => root.render(<Reasons unmet={unmet} />));
  const value = Array.from(host.querySelectorAll('.reasons .mono')).find((e) => e.textContent === changes);
  const lines = value?.getClientRects().length ?? 0;
  root.unmount();
  host.remove();
  return lines;
}

describe('a reason a return was sent back for', () => {
  it('keeps each line of a value recorded of it on a line of its own', () => {
    expect(linesOf(' M a.rs')).toBe(1);
    expect(linesOf(' M a.rs\n M b.rs\n?? c.rs')).toBe(3);
  });
});
