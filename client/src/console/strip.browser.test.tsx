// The strip on a console's card, in a real browser (Chromium): the canvas draws the works page's marks, so each mark it
// drew, as the scene describes it (`drawnStrip`), is held against the works page's own `Strip` laid out by the browser
// in the same tokens: its size, ring, corners, fill and place, state by state, in light and in dark, and drawn again,
// with no reload, in an accent picked. And a flow with more cells than the card has room for keeps the marks' size.

import '../styles/tokens.css';
import '../styles/parts.css';
import '../styles/works.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { pickAccent, PRESETS, tones } from '../accent';
import { Strip } from '../works/marks';
import type { CellNow, CellState } from '../works-api';
import type { Shown } from './items';
import { type DrawnStrip, SnapScene } from './snap';

const STATES: CellState[] = ['done', 'running', 'calling', 'resting', 'failed', 'again', 'notYet', 'skipped', 'notRun'];

const cellsOf = (states: CellState[]): CellNow[] =>
  states.map((state, i) => ({
    cell: `c${i}`,
    state,
    finished: state === 'done' ? 1 : 0,
    record: null,
    open: state === 'running' || state === 'calling' || state === 'resting' || state === 'failed' ? 1 : null,
  }));

/// An item whose work, a round under way at level 2, stands as `cells` say.
const working = (id: number, cells: CellNow[]): Shown => ({
  id,
  title: `Item ${id}`,
  standing: 'open',
  asks: 0,
  deliver: false,
  blocked: 0,
  unseen: false,
  unread: 0,
  begun: true,
  shadow: false,
  work: {
    number: id,
    now: { kind: 'round', cell: 'c1', run: 1, round: 1, since: Date.now() },
    paused: null,
    waits: [],
    bar: '',
    sentBack: 0,
    cells,
    from: Date.now(),
    usdMicros: 12_340_000,
    last: null,
    level: '2',
    raise: 0,
    lines: null,
  },
  children: [],
});

const frame = () => new Promise<void>((done) => requestAnimationFrame(() => done()));

/// `#rrggbb` as the browser gives a colour, `rgb(r, g, b)`.
const rgb = (hex: string) => {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`;
};

describe("a card's strip", () => {
  let host: HTMLDivElement;
  let page: HTMLDivElement;
  let scene: SnapScene;
  beforeEach(async () => {
    host = document.createElement('div');
    host.style.cssText = 'position: fixed; left: 0; top: 0; width: 1000px; height: 640px';
    page = document.createElement('div');
    document.body.append(host, page);
    scene = new SnapScene();
    await scene.mount(host);
  });
  afterEach(() => {
    scene.destroy();
    host.remove();
    page.remove();
    document.documentElement.removeAttribute('data-theme');
    pickAccent(null);
  });

  /// The works page's strip of `cells`, laid out by the browser: each mark's place from the first's left and from the
  /// strip's middle, and its computed look.
  function laidOut(cells: CellNow[]) {
    const root = createRoot(page);
    flushSync(() => root.render(<Strip cells={cells} />));
    const strip = page.querySelector('.strip') as HTMLElement;
    const box = strip.getBoundingClientRect();
    const marks = [...strip.querySelectorAll('.state-mark')].map((m) => {
      const r = m.getBoundingClientRect();
      const s = getComputedStyle(m);
      return {
        x: r.left - box.left,
        y: r.top - (box.top + box.height / 2),
        width: r.width,
        height: r.height,
        corner: Number.parseFloat(s.borderTopLeftRadius),
        ring: Number.parseFloat(s.borderTopWidth),
        edge: s.borderTopStyle === 'none' ? null : s.borderTopColor,
        fill: s.backgroundColor,
      };
    });
    root.unmount();
    return marks;
  }

  /// The canvas's strip in the same terms: each mark from the first's left and from the row's middle.
  function drawn(strip: DrawnStrip) {
    const left = strip.marks[0].x;
    const middle = strip.marks[0].y + strip.marks[0].height / 2;
    return strip.marks.map((m) => ({
      x: m.x - left,
      y: m.y - middle,
      width: m.width,
      height: m.height,
      corner: m.corner,
      ring: m.ring,
      edge: m.edge === null ? null : rgb(m.edge),
      fill: m.fill === null ? 'rgba(0, 0, 0, 0)' : rgb(m.fill),
    }));
  }

  it("draws each cell's state as the works page's mark, in light and in dark", async () => {
    const cells = cellsOf(STATES);
    scene.show([working(1, cells)]);
    const light = scene.drawnStrip(1) as DrawnStrip;
    expect(light.marks.map((m) => m.state)).toEqual(STATES);
    expect(light.more).toBe(0);
    expect(drawn(light)).toEqual(laidOut(cells));
    // Only the marks of the cell running and of the cell with a call under way pulse.
    expect(light.marks.map((m) => m.pulses)).toEqual([false, true, true, false, false, false, false, false, false]);
    // Under the number, the level the work's cell runs at, in the look of that level: the second in the text's colour
    // (decision a-level-looks-as-the-level-it-runs-at).
    const text = getComputedStyle(document.documentElement).getPropertyValue('--text').trim();
    expect(light.level).toEqual({ text: 'L2', colour: text, strong: false });
    // A filled mark is not ringed in its own colour, which would show as a border on a dimmed card; a hollow one is.
    expect(light.marks.map((m) => m.ringed)).toEqual([false, false, false, false, true, true, true, false, false]);
    // Under the title, so that the number stands alone at the card's left, and short of where the work's time went.
    expect(light.marks[0].x).toBe(36);
    const last = light.marks[light.marks.length - 1];
    expect(last.x + last.width).toBeLessThanOrEqual(light.end);

    document.documentElement.setAttribute('data-theme', 'dark');
    await expect.poll(() => scene.drawnStrip(1)).not.toBe(light);
    await frame();
    const dark = scene.drawnStrip(1) as DrawnStrip;
    expect(dark.marks[0].fill).not.toBe(light.marks[0].fill);
    expect(drawn(dark)).toEqual(laidOut(cells));
  });

  it('draws the marks again in an accent picked, with no reload, as the works page draws them, in light and in dark', async () => {
    const cells = cellsOf(STATES);
    const violet = PRESETS[2].colour;
    scene.show([working(1, cells)]);
    const before = scene.drawnStrip(1) as DrawnStrip;
    expect(pickAccent(violet)).toBeNull();
    await expect.poll(() => scene.drawnStrip(1)).not.toBe(before);
    await frame();
    const light = scene.drawnStrip(1) as DrawnStrip;
    expect(drawn(light)).toEqual(laidOut(cells));
    const done = light.marks.find((m) => m.state === 'done');
    const running = light.marks.find((m) => m.state === 'running');
    expect(done?.fill).toBe(tones(violet).light['mark-done']);
    expect(running?.fill).toBe(tones(violet).light['mark-running']);

    document.documentElement.setAttribute('data-theme', 'dark');
    await expect.poll(() => scene.drawnStrip(1)).not.toBe(light);
    await frame();
    const dark = scene.drawnStrip(1) as DrawnStrip;
    expect(drawn(dark)).toEqual(laidOut(cells));
    expect(dark.marks.find((m) => m.state === 'done')?.fill).toBe(tones(violet).dark['mark-done']);
  });

  it("fits the engineer's ten cells at the marks' full size, and past the card's room draws those that fit and says how many more", () => {
    const ten = cellsOf(Array.from({ length: 10 }, (_, i) => STATES[i % STATES.length]));
    scene.show([working(1, ten), working(2, cellsOf(Array.from({ length: 40 }, () => 'notYet')))]);
    const fits = scene.drawnStrip(1) as DrawnStrip;
    expect(fits.marks).toHaveLength(10);
    expect(fits.more).toBe(0);
    expect(drawn(fits)).toEqual(laidOut(ten));

    const long = scene.drawnStrip(2) as DrawnStrip;
    expect(long.more).toBeGreaterThan(0);
    expect(long.marks.length + long.more).toBe(40);
    expect(long.marks.map((m) => m.cell)).toEqual(Array.from({ length: long.marks.length }, (_, i) => `c${i}`));
    // The marks keep the works page's size and gaps.
    expect(long.marks.every((m) => m.width === fits.marks[0].width)).toBe(true);
    expect(long.marks[1].x - long.marks[0].x).toBe(fits.marks[1].x - fits.marks[0].x);
    const last = long.marks[long.marks.length - 1];
    expect(last.x + last.width).toBeLessThanOrEqual(long.end);
  });
});
