// The spans a view counts over, as links, in a real browser (Chromium): the one in use stands over a line of the accent
// two pixels thick, and the others over none. The line is a border the `.link` rule, which clears every border, would
// take away where it out-weighed the span's own, as it did once, which only the browser's cascade shows.

import './styles/tokens.css';
import './styles/parts.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { SpanLinks } from './parts';

/// The colour the browser draws a token in, as it gives a colour.
function drawnIn(token: string): string {
  const probe = document.createElement('span');
  probe.style.color = `var(${token})`;
  document.body.append(probe);
  const colour = getComputedStyle(probe).color;
  probe.remove();
  return colour;
}

describe('the span links', () => {
  let host: HTMLDivElement;
  afterEach(() => host.remove());

  it('draw the span in use over a line in the accent, two pixels thick, and the others over none', () => {
    host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    flushSync(() =>
      root.render(
        <SpanLinks
          label="The span the numbers count over"
          value="4w"
          choices={[
            { value: 'week', name: 'This week', href: '#/system/week' },
            { value: '4w', name: '4 weeks', href: '#/system/4w' },
            { value: 'all', name: 'All time', href: '#/system/all' },
          ]}
        />,
      ),
    );
    const [week, current, all] = [...host.querySelectorAll<HTMLAnchorElement>('a.span-link')];
    const line = (a: HTMLElement) => {
      const style = getComputedStyle(a);
      return { width: style.borderBottomWidth, style: style.borderBottomStyle, colour: style.borderBottomColor };
    };
    expect(current.getAttribute('aria-current')).toBe('true');
    expect(line(current)).toEqual({ width: '2px', style: 'solid', colour: drawnIn('--accent') });
    // The others hold the room of the line, so that choosing a span moves nothing, and draw none.
    for (const other of [week, all]) {
      expect(line(other)).toMatchObject({ width: '2px', style: 'solid', colour: 'rgba(0, 0, 0, 0)' });
    }
    root.unmount();
  });
});
