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

/// The height the browser lays out the part of a reason that holds the value `changes` in, drawn as the Step draws it:
/// a block with one line box for each line of the value, all of the same height.
function heightOf(changes: string): number {
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  const unmet = [{ key: 'uncommitted', values: [{ name: 'changes', value: changes }] }];
  flushSync(() => root.render(<Reasons unmet={unmet} />));
  const value = Array.from(host.querySelectorAll('.reasons .mono')).find((e) => e.textContent === changes);
  const height = value?.parentElement?.getBoundingClientRect().height ?? 0;
  root.unmount();
  host.remove();
  return height;
}

describe('a reason a return was sent back for', () => {
  it('keeps each line of a value recorded of it on a line of its own', () => {
    const oneLine = heightOf(' M a.rs');
    expect(oneLine).toBeGreaterThan(0);
    expect(Math.round(heightOf(' M a.rs\n M b.rs\n?? c.rs') / oneLine)).toBe(3);
  });
});
