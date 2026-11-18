// A diagram an agent writes in a passage (the design of the owner's page, 9 to 13), in a real browser (Chromium), whose
// parsing of SVG and shadow roots the cleaning and the drawing rest on: a ```mermaid or ```dot fence drawn as a diagram
// once it comes near the screen, at mermaid's strictest setting and cleaned of what draws no SVG; and, where it cannot
// be drawn, its source with a note under it. The libraries are stood in for: what they draw is theirs, and what is
// tested is what the page does with it; and the screen is stood in for, to say when a diagram comes near it.
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Markdown } from './console/markdown';

/// A drawing as a library gives it: an SVG with its own box, a script and a foreign object in it, which the page drops.
const svg = (id: string) =>
  `<svg id="${id}" viewBox="0 0 120 40"><script>window.ran = true</script>` +
  `<foreignObject><div>html</div></foreignObject><g><text>drawn</text></g></svg>`;

/// What Graphviz gives back of a drawing.
type Graph = { status: 'success' | 'failure'; output: string; errors: { message: string }[] };

const libraries = vi.hoisted(() => ({
  mermaid: {
    initialize: vi.fn(),
    render: vi.fn<(id: string, source: string) => Promise<{ svg: string }>>(),
  },
  viz: { render: vi.fn<(source: string, options: unknown) => Graph>() },
}));

vi.mock('mermaid', () => ({ default: libraries.mermaid }));
vi.mock('@viz-js/viz', () => ({ instance: async () => libraries.viz }));

/// The places observed for coming near the screen, which a test brings near it.
let watched: { callback: IntersectionObserverCallback; targets: Element[] }[] = [];

beforeEach(() => {
  watched = [];
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      private entry: { callback: IntersectionObserverCallback; targets: Element[] };
      constructor(callback: IntersectionObserverCallback) {
        this.entry = { callback, targets: [] };
        watched.push(this.entry);
      }
      observe(target: Element) {
        this.entry.targets.push(target);
      }
      disconnect() {
        this.entry.targets = [];
      }
    },
  );
  libraries.mermaid.initialize.mockClear();
  libraries.mermaid.render.mockReset();
  libraries.mermaid.render.mockImplementation(async (id: string) => ({ svg: svg(id) }));
  libraries.viz.render.mockReset();
  libraries.viz.render.mockImplementation(() => ({
    status: 'success',
    output: `<?xml version="1.0"?>\n${svg('graph0')}`,
    errors: [],
  }));
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

function draw(text: string): HTMLElement {
  const container = document.body.appendChild(document.createElement('div'));
  const root = createRoot(container);
  act(() => root.render(<Markdown text={text} />));
  return container;
}

/// Brings every place observed near the screen.
function nearTheScreen() {
  act(() => {
    for (const w of watched) {
      if (w.targets.length > 0) {
        w.callback(
          w.targets.map((target) => ({ target, isIntersecting: true }) as IntersectionObserverEntry),
          {} as IntersectionObserver,
        );
      }
    }
  });
}

/// Waits until `test` holds, giving what the page set going (a library loaded, a drawing cleaned) its turns.
async function until(test: () => boolean) {
  for (let i = 0; i < 200 && !test(); i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 5));
    });
  }
  expect(test()).toBe(true);
}

/// The drawing of the diagram in this place, as its shadow root holds it.
const drawing = (el: HTMLElement) => el.querySelector('.diagram-drawing')?.shadowRoot?.innerHTML ?? '';

describe('a diagram in what an agent wrote', () => {
  it('is drawn from a ```mermaid fence once it comes near the screen, at the strictest setting, with nothing that draws no SVG', async () => {
    const source = 'flowchart LR\n  plan --> build';
    const el = draw(`Before.\n\n\`\`\`mermaid\n${source}\n\`\`\`\n\nAfter.`);
    // Until it comes near the screen, its source stands in its place and the library is not asked.
    expect(el.querySelector('.diagram pre code')?.textContent).toBe(source);
    expect(libraries.mermaid.render).not.toHaveBeenCalled();
    nearTheScreen();
    await until(() => drawing(el) !== '');
    expect(libraries.mermaid.render).toHaveBeenCalledTimes(1);
    expect(libraries.mermaid.render.mock.calls[0][1]).toBe(source);
    expect(libraries.mermaid.initialize).toHaveBeenCalledWith(
      expect.objectContaining({ securityLevel: 'strict', htmlLabels: false, startOnLoad: false }),
    );
    expect(drawing(el)).toContain('<text>drawn</text>');
    expect(drawing(el)).not.toContain('<script');
    expect(drawing(el)).not.toContain('foreignObject');
    // At the size it was drawn, as its own box gives it.
    expect(drawing(el)).toContain('width="120"');
    expect(drawing(el)).toContain('height="40"');
    expect(el.querySelector('.diagram pre')).toBeNull();
    expect(el.querySelector('.diagram-note')).toBeNull();
    expect(el.textContent).toContain('Before.');
    expect(el.textContent).toContain('After.');
  });

  it('shows its source in its place at a press, and the diagram again at another', async () => {
    const source = 'flowchart TD\n  a --> b';
    const el = draw(`\`\`\`mermaid\n${source}\n\`\`\``);
    nearTheScreen();
    await until(() => drawing(el) !== '');
    const toggle = () =>
      [...el.querySelectorAll<HTMLButtonElement>('.diagram-tool')].find((b) => b.getAttribute('aria-pressed') !== null);
    expect(toggle()?.textContent).toBe('Source');
    act(() => toggle()?.click());
    expect(el.querySelector('.diagram pre code')?.textContent).toBe(source);
    expect(el.querySelector('.diagram-drawing')).toBeNull();
    expect(toggle()?.textContent).toBe('Diagram');
    act(() => toggle()?.click());
    expect(el.querySelector('.diagram-drawing')).not.toBeNull();
  });

  it('is drawn from a ```dot fence by Graphviz, cleaned the same way', async () => {
    const source = 'digraph { plan -> build }';
    const el = draw(`\`\`\`dot\n${source}\n\`\`\``);
    nearTheScreen();
    await until(() => drawing(el) !== '');
    expect(libraries.viz.render).toHaveBeenCalledTimes(1);
    expect(libraries.viz.render.mock.calls[0][0]).toBe(source);
    expect(drawing(el)).toContain('<text>drawn</text>');
    expect(drawing(el)).not.toContain('<script');
    expect(drawing(el)).not.toContain('<?xml');
  });

  it('that cannot be drawn shows its source with a note under it, never an empty place', async () => {
    libraries.mermaid.render.mockImplementation(async () => {
      throw new Error('Parse error on line 1');
    });
    const mermaid = draw('```mermaid\nflowchart LR\n  a -->\n```');
    libraries.viz.render.mockImplementation(() => ({
      status: 'failure',
      output: '',
      errors: [{ message: 'syntax error in line 1' }],
    }));
    const dot = draw('```dot\ndigraph { a -> }\n```');
    nearTheScreen();
    for (const [el, source] of [
      [mermaid, 'flowchart LR\n  a -->'],
      [dot, 'digraph { a -> }'],
    ] as const) {
      await until(() => el.querySelector('.diagram-note') !== null);
      expect(el.querySelector('.diagram pre code')?.textContent).toBe(source);
      expect(el.querySelector('.diagram-note')?.textContent).toBe('This diagram could not be drawn; its source is above.');
      expect(el.querySelector('.diagram-drawing')).toBeNull();
    }
  });

  it('is drawn only for a fence of a diagram’s kind, in any case; another fence is code', () => {
    const el = draw('```Mermaid\ngraph LR\n```\n\n```text\ngraph LR\n```');
    expect(el.querySelectorAll('.diagram')).toHaveLength(1);
    expect(el.querySelectorAll('pre.content code')).toHaveLength(2);
  });
});

describe('a block of code in what an agent wrote', () => {
  it("is coloured by the language its fence names, and drawn in the text's colour where the page knows none", () => {
    const el = draw('```rust\nlet ms = 1000;\n```\n\n```cobol\nlet ms = 1000;\n```');
    const [rust, other] = [...el.querySelectorAll('pre.content')];
    expect([...rust.querySelectorAll('[class^="code-"]')].map((s) => `${s.className}:${s.textContent}`)).toEqual([
      'code-keyword:let',
      'code-literal:1000',
    ]);
    expect(other.querySelector('[class^="code-"]')).toBeNull();
    expect(other.textContent).toBe('let ms = 1000;');
  });
});
