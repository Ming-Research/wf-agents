// @vitest-environment happy-dom
// What the canvas draws of a work on its card, apart from drawing it: the colour of its age, how its rings move, and
// who moves it next, which its card's colour says; what a fold hides, which counts at the item folded; where each item of
// the tree stands; how large the items near the pointer are drawn; and how an item's brightness goes up and down.

import { describe, expect, it } from 'vitest';

import type { CellNow, Now } from '../works-api';
import { stateOf } from '../works/words';
import { age, type Shown } from './items';
import { indexTree } from './tree';
import {
  beatOf,
  beneath,
  dimmed,
  DOT_AT,
  DOT_R,
  FOLD_MS,
  FOLD_PAD,
  FOLD_REACH,
  foldWords,
  inFunnel,
  sizeOf,
  type Turn,
  turnEase,
  UNFOLD_EASE,
  UNFOLD_MS,
  DIM,
  eased,
  gatherAt,
  headAt,
  layOut,
  lensSize,
  lightOn,
  linksStrength,
  NODE_W,
  PITCH,
  ROW,
  sparkAt,
  strandOf,
  tonesOf,
  wayAims,
  waysUp,
} from './snap';

const MINUTE = 60_000;

describe("a work's age on its card", () => {
  it('takes three colours and no more: grey under half an hour, amber to an hour, red past an hour however long', () => {
    expect(age(10 * MINUTE).tone).toBe('success');
    expect(age(45 * MINUTE).tone).toBe('attention');
    expect(age(90 * MINUTE).tone).toBe('danger');
    expect(age(30 * 60 * MINUTE).tone).toBe('danger');
    expect(age(3 * 24 * 60 * MINUTE)).toEqual({ label: '3d', tone: 'danger' });
  });
});

describe('what a fold hides', () => {
  const item = (id: number, children: Shown[] = [], shadow = false): Shown => ({
    id,
    title: `Item ${id}`,
    standing: 'open',
    asks: 0,
    ahead: 0,
    deliver: false,
    fresh: 'none',
    repliesUnread: 0,
    begun: false,
    underWay: false,
    shadow,
    stands: null,
    work: null,
    children,
  });
  // 1 holds 2, which holds 3, folded away under 2, and a shadow of 4.
  const folded = { folded: true, under: { open: 1, done: 0, cancelled: 0, elsewhere: 0 } };
  const tree = item(1, [{ ...item(2, [item(3), item(4, [], true)]), fold: folded }]);

  it('is what stands under an item at any depth, folded away or not, and no shadow, whose item stands elsewhere', () => {
    expect(beneath(tree, (s) => s.id === 3)).toBe(true);
    expect(beneath(tree, (s) => s.id === 2)).toBe(true);
    expect(beneath(tree, (s) => s.id === 4)).toBe(false);
    expect(beneath(tree, (s) => s.id === 1)).toBe(false);
  });
});

describe('a fold', () => {
  it("folds into a dot 9 pixels across, its middle 12 pixels past the card's edge, within the room a press on it takes", () => {
    expect({ across: 2 * DOT_R, out: DOT_AT }).toEqual({ across: 9, out: 12 });
    // The dot, and the disc of the pointer's tint round it, lie past the card's edge and within the room of a press.
    expect(DOT_AT - DOT_R - FOLD_PAD).toBeGreaterThan(0);
    expect(DOT_AT + DOT_R + FOLD_PAD).toBeLessThanOrEqual(FOLD_REACH);
  });

  it("turns in the page's motion for opening and closing a part: from none of the way to all of it, most of it early, never back and never past its end", () => {
    expect([turnEase(-1), turnEase(0), turnEase(1), turnEase(2)]).toEqual([0, 0, 1, 1]);
    // cubic-bezier(0.2, 0.8, 0.2, 1): half its time gone, some 95% of the way.
    expect(turnEase(0.5)).toBeCloseTo(0.946, 2);
    for (let t = 0; t < 1; t += 0.05) expect(turnEase(t + 0.05)).toBeGreaterThanOrEqual(turnEase(t));
  });

  it('grows on the page\'s curve, and shrinks on a gentle ease-out of its own: at once from its first frame, and all the way to the dot', () => {
    const grows: Turn = { to: 1, from: 0, at: 0, ms: UNFOLD_MS };
    const shrinks: Turn = { to: 0, from: 1, at: 0, ms: FOLD_MS };
    // An unfold keeps the page's curve: half its time gone, some 95% of its size.
    expect(sizeOf(grows, UNFOLD_MS / 2)).toBeCloseTo(turnEase(0.5, UNFOLD_EASE), 9);
    expect(sizeOf(grows, UNFOLD_MS / 2)).toBeCloseTo(0.946, 2);
    // A fold, in less time, moves in its first frame, by well under a quarter, shrinks at every frame after, and is
    // still drawn at half its time.
    expect(FOLD_MS).toBeLessThan(UNFOLD_MS);
    expect(sizeOf(shrinks, 16)).toBeLessThan(0.95);
    expect(sizeOf(shrinks, 16)).toBeGreaterThan(0.8);
    for (let at = 16; at <= FOLD_MS; at += 16) expect(sizeOf(shrinks, at)).toBeLessThan(sizeOf(shrinks, at - 16));
    expect(sizeOf(shrinks, FOLD_MS / 2)).toBeGreaterThan(0.2);
    expect([sizeOf(shrinks, 0), sizeOf(shrinks, FOLD_MS), sizeOf(shrinks, 999)]).toEqual([1, 0, 0]);
    // A fold reversed part of the way goes on from the size it had reached.
    const back: Turn = { to: 0, from: 0.6, at: 0, ms: FOLD_MS * 0.6 };
    expect(sizeOf(back, 0)).toBe(0.6);
    expect(sizeOf(back, 16)).toBeLessThan(0.6);
  });

  it('is pressed in its funnel, between the curves the funnel is drawn with, and nowhere past either end of it', () => {
    // A card 26 pixels high ending at 300, the items under it starting at 370, from 100 to 200 down the screen. Half
    // way across, each curve has gone half way down: from -13 to 43.5, and from 13 to 106.5.
    const at = (x: number, y: number) => inFunnel(300, -13, 13, 370, 100, 200, x, y);
    expect([at(335, 44), at(335, 106), at(301, 0), at(369, 150)]).toEqual([true, true, true, true]);
    expect([at(335, 43), at(335, 107), at(369, 0), at(299, 0), at(371, 150)]).toEqual(Array(5).fill(false));
  });

  it('says on pointing what a click on it does, then how many items stand under it by kind and how many shadows, any with none left out', () => {
    expect(foldWords({ folded: false, under: { open: 2, done: 4, cancelled: 0, elsewhere: 0 } })).toBe(
      'Fold the items under it away (e): open 2, done 4',
    );
    expect(foldWords({ folded: true, under: { open: 0, done: 1, cancelled: 2, elsewhere: 0 } })).toBe(
      'Show the items under it (e): done 1, cancelled 2',
    );
    // The shadows under it are counted apart, after the items, as their items are shown elsewhere.
    expect(foldWords({ folded: false, under: { open: 0, done: 0, cancelled: 0, elsewhere: 1 } })).toBe(
      'Fold the items under it away (e): 1 shown elsewhere',
    );
    expect(foldWords({ folded: false, under: { open: 2, done: 0, cancelled: 0, elsewhere: 1 } })).toBe(
      'Fold the items under it away (e): open 2, 1 shown elsewhere',
    );
  });
});

describe('the lit ways of the items whose work runs', () => {
  // Item 0 is a root with 1 and 2 under it, and 3 under 1; item 4 is a root of its own.
  const parent = Int32Array.from([-1, 0, 0, 1, -1]);
  const ways = (running: number[]) => [...waysUp(running, parent)].sort((a, b) => a - b);

  it('take each link from a running item up to its root once, however many running items it is on', () => {
    expect(ways([3])).toEqual([1, 3]);
    expect(ways([3, 2])).toEqual([1, 2, 3]);
    expect(ways([3, 1])).toEqual([1, 3]);
  });

  it('take no link above a root', () => {
    expect(ways([0, 4])).toEqual([]);
  });

  it('stand at full strength while nothing is lit, and while something is, only where both ends are lit', () => {
    const aims = (on: number[], lit: number[]) =>
      Object.fromEntries([...wayAims(on, parent, new Set(lit))].map(([i, a]) => [i, a === 1 ? 'full' : 'dim']));
    expect(aims([1, 2, 3], [])).toEqual({ 1: 'full', 2: 'full', 3: 'full' });
    // Item 1 lit, with its root and the item under it.
    expect(aims([1, 2, 3], [1, 0, 3])).toEqual({ 1: 'full', 2: 'dim', 3: 'full' });
    // Item 2 lit with its root: the link from 3 joins nothing lit, and the link from 1 has only one end lit.
    expect(aims([1, 2, 3], [2, 0])).toEqual({ 1: 'dim', 2: 'full', 3: 'dim' });
    // Dimmed, a way is as strong as the links it lies on once the items are dimmed.
    expect(wayAims([1], parent, new Set([4])).get(1)).toBe(linksStrength(DIM));
    // While the search is open the tree dims with nothing lit, and every way with it.
    expect(wayAims([1], parent, new Set(), true).get(1)).toBe(linksStrength(DIM));
    expect(linksStrength(1)).toBe(1);
  });

  it('ease to their strength over frames, down and back up in the same time, and come to it, as the items do', () => {
    const frame = 16;
    const dimmed = linksStrength(DIM);
    // One frame after the light goes off a way, it has gone only a little of the way down.
    const once = eased(1, dimmed, frame);
    expect(once).toBeLessThan(1);
    expect(once).toBeGreaterThan(1 - (1 - dimmed) / 4);
    // Coming back up goes as far in a frame as going down.
    expect(eased(dimmed, 1, frame) - dimmed).toBeCloseTo(1 - once, 12);
    let a = dimmed;
    for (let n = 0; n < 120; n += 1) a = eased(a, 1, frame);
    expect(a).toBeCloseTo(1, 3);
    expect(eased(0.7, 0.7, frame)).toBe(0.7);
  });

  it('carry one light down from the root, brightest at its head, cut at the ends of each stretch it is on', () => {
    // A stretch 100 long that begins 300 down from the root, with the light's head 340 down.
    const on = lightOn(300, 100, 340);
    // The light stands where it has run to, its head the brightest of it, and none of it is drawn off the stretch.
    expect(on.at(-1)?.to).toBeCloseTo(40, 9);
    expect(on.at(-1)?.bright).toBe(1);
    expect([...on.map((piece) => piece.bright)].sort((a, b) => a - b)).toEqual(on.map((piece) => piece.bright));
    for (const { from, to } of [...on, ...lightOn(300, 100, 310), ...lightOn(300, 100, 460)]) {
      expect(from).toBeGreaterThanOrEqual(0);
      expect(to).toBeLessThanOrEqual(100);
      expect(to).toBeGreaterThan(from);
    }
    // It moves on down the way as the light runs, and the stretches it has not reached or has gone past carry none.
    expect(lightOn(300, 100, 360).at(-1)?.to).toBeGreaterThan(on.at(-1)?.to ?? 0);
    expect(lightOn(300, 100, 280)).toEqual([]);
    expect(lightOn(300, 100, 600)).toEqual([]);
    // A stretch with no length carries none.
    expect(lightOn(300, 0, 340)).toEqual([]);
  });

  it('run slow at the start of a run and faster towards its end', () => {
    expect(headAt(0, 1000)).toBe(0);
    expect(headAt(1, 1000)).toBe(1000);
    // Half the run gone, less than half the way is behind it, and it goes further in its last tenth than its first.
    expect(headAt(0.5, 1000)).toBeLessThan(500);
    expect(headAt(1, 1000) - headAt(0.9, 1000)).toBeGreaterThan(headAt(0.1, 1000) - headAt(0, 1000));
  });

  it('burn the spark where a way meets a card up to the full as the light goes through it', () => {
    // A spark 300 down from the root: at rest before the light has come and once it has gone, and at the full as
    // the light's head goes through it.
    expect(sparkAt(300, 300)).toBe(1);
    expect(sparkAt(300, 100)).toBeLessThan(1);
    expect(sparkAt(300, 100)).toBe(sparkAt(300, 600));
    expect(sparkAt(300, 300)).toBeGreaterThan(sparkAt(300, 240));
  });

  it('gather the strands into the line where the way meets a card, and run them beside it between', () => {
    expect(gatherAt(0)).toBe(0);
    expect(gatherAt(1)).toBeCloseTo(0, 6);
    expect(gatherAt(0.5)).toBe(1);
    expect(gatherAt(0.3)).toBeCloseTo(gatherAt(0.7), 9);
    // Nearly level along the middle, so that they do not swell and narrow between the ends.
    expect(gatherAt(0.25)).toBeGreaterThan(0.8);
  });

  it('scatter the strands across both sides of the line, none together, the outer ones fainter', () => {
    const strands = Array.from({ length: 14 }, (_, s) => strandOf(s));
    const sides = strands.map(({ side }) => side).sort((a, b) => a - b);
    expect(sides[0]).toBeLessThan(0);
    expect(sides.at(-1)).toBeGreaterThan(0);
    for (let n = 1; n < sides.length; n += 1) expect(sides[n] - sides[n - 1]).toBeGreaterThan(0.01);
    for (const { side, own } of strands) {
      expect(Math.abs(side)).toBeLessThan(1);
      expect(own).toBeGreaterThan(0);
      expect(own).toBeLessThanOrEqual(1 - 0.5 * Math.abs(side));
    }
  });
});

describe('who moves an item next, as its card says', () => {
  const shown = (id: number, now: Now | null, children: Shown[] = [], more: Partial<Shown> = {}): Shown => ({
    id,
    title: `Item ${id}`,
    standing: 'open',
    asks: 0,
    ahead: 0,
    deliver: false,
    fresh: 'none',
    repliesUnread: 0,
    begun: now !== null,
    underWay: false,
    shadow: false,
    stands: now === null ? null : { now, cells: [], waits: [] },
    work: null,
    children,
    ...more,
  });
  const round: Now = { kind: 'round', cell: 'implement', run: 1, round: 1, since: 0 };
  const call: Now = {
    kind: 'waits',
    at: 'implement',
    on: [{ kind: 'call', call: 1, who: null, what: { kind: 'tool', name: 'ci', input: 'the tests' } }],
  };
  const turn: Now = { kind: 'waits', at: 'merge', on: [{ kind: 'turn', holder: 3, since: 0 }] };
  const items: Now = { kind: 'waits', at: 'deliver', on: [{ kind: 'items', items: [9] }] };
  const paused: Now = { kind: 'paused', ran: true, at: 'implement', by: 'owner', why: null };
  const fault: Now = { kind: 'fault', words: 'No session' };
  const broken: Now = { kind: 'broken', words: 'the driver names no turn' };
  const asks = { key: 'answer-which', why: 'Which?', ends: { kind: 'resume' as const } };

  it('takes the tone of the state every view names a work by, a call waited for being blue while the engine carries it out and yellow while it waits its turn', () => {
    const tone = (now: Now, cells: CellNow[] = []) => {
      const item = { ...shown(1, now), stands: { now, cells, waits: [] } };
      return tonesOf([item]).tone.get(item);
    };
    // A work that waits for the turn, whatever call is under way at another cell: the state says it waits, and its
    // card waits its turn.
    const atM1: CellNow[] = [{ cell: 'M1', state: 'calling', finished: 0, record: null, open: 1 }];
    const turnAt: Now = { kind: 'waits', at: 'M2', on: [{ kind: 'turn', holder: 3, since: 0 }] };
    expect(stateOf({ now: turnAt, cells: atM1, waits: [] }).which).toBe('waiting');
    expect(tone(turnAt, atM1)).toBe('yellow');
    // A call of a tool the engine carries out, and the act of an engine cell under way, its cell marked as calling:
    // the system works on it.
    const atM2: CellNow[] = [{ ...atM1[0], cell: 'M2' }];
    const tool: Now = {
      kind: 'waits',
      at: 'M2',
      on: [{ kind: 'call', call: 4, who: null, what: { kind: 'tool', name: 'run_tests', input: '' } }],
    };
    expect(stateOf({ now: tool, cells: atM2, waits: [] }).which).toBe('calling');
    expect(tone(tool, atM2)).toBe('blue');
    const act: Now = { kind: 'round', cell: 'M2', run: 2, round: 1, since: 0 };
    expect(tone(act, atM2)).toBe('blue');
    expect(tone(act, [{ ...atM2[0], state: 'running' }])).toBe('blue');
    // The same call waiting for a runner, nothing carried out yet: it waits its turn.
    const queued: Now = {
      kind: 'waits',
      at: 'M2',
      on: [
        {
          kind: 'call',
          call: 4,
          who: null,
          what: { kind: 'tool', name: 'run_tests', input: '' },
          queuedFor: 'a runner',
        },
      ],
    };
    expect(tone(queued, atM2)).toBe('yellow');
    // A pause, the owner's or of everything, does not move by itself.
    expect(tone({ kind: 'paused', ran: true, at: 'M1', by: 'owner', why: null })).toBe('grey');
    expect(tone({ kind: 'everything', ran: true, at: 'M1', by: 'owner' })).toBe('grey');
  });

  it('is the owner where its work waits for the owner or was stopped, the system where a round or a call runs, its turn where it waits for one, and no one where it does not move by itself', () => {
    const owner = { ...shown(1, round), stands: { now: round, cells: [], waits: [asks] } };
    const roots = [
      owner,
      shown(2, fault),
      shown(3, broken),
      shown(4, round),
      shown(5, call),
      shown(6, turn),
      shown(7, paused),
      shown(8, items),
      shown(9, null),
      shown(10, null, [], { standing: 'done' }),
      shown(11, round, [], { standing: 'cancelled' }),
      shown(12, round, [], { shadow: true }),
    ];
    const { tone, moves } = tonesOf(roots);
    expect(roots.map((r) => tone.get(r))).toEqual([
      'red',
      'red',
      'red',
      'blue',
      'blue',
      'yellow',
      'grey',
      'grey',
      'grey',
      'green',
      'grey',
      'grey',
    ]);
    // What moves of its own, and so glows: what the owner, the system or its turn moves.
    expect(roots.filter((r) => moves.has(r)).map((r) => r.id)).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it('gives an item that waits for the items under it the most pressing tone under it, blue before red before yellow, and grey where nothing under it moves or only a shadow does; a paused item stays grey', () => {
    const red = shown(2, fault);
    const blue = shown(3, round);
    const yellow = shown(4, turn);
    // Under 1, a paused item over a red, and a yellow; under 5, a red and a yellow under an item of its own, and a
    // blue; under 6 a grey; under 7 a shadow of a running item, which is drawn elsewhere.
    const twenty = shown(20, paused, [red]);
    const twentyOne = shown(21, null, [red, yellow]);
    const one = shown(1, items, [twenty, yellow]);
    const five = shown(5, null, [twentyOne, blue]);
    const six = shown(6, null, [shown(22, paused)]);
    const seven = shown(7, null, [shown(23, round, [], { shadow: true })]);
    const { tone, moves } = tonesOf([one, five, six, seven]);
    expect([one, twenty, five, twentyOne, six, seven, yellow].map((s) => tone.get(s))).toEqual([
      'yellow',
      'grey',
      'blue',
      'red',
      'grey',
      'grey',
      'yellow',
    ]);
    // An item takes its tone from under it, but moves only of its own.
    expect([one, five, twentyOne, red, blue, yellow].map((s) => moves.has(s))).toEqual([
      false,
      false,
      false,
      true,
      true,
      true,
    ]);
  });
});

describe('where each item of the tree stands', () => {
  type Node = { id: number; under: Node[] };
  const node = (id: number, under: Node[] = []): Node => ({ id, under });
  /// The tree of `roots` laid out, each item as many rows high as `rowsOf` says, and the items under an item drawn at
  /// the part of their size `sizeOf` says of it, as a fold turning draws them: where each item starts and how many rows
  /// it takes, by its number, and the tree.
  function laid(roots: Node[], rowsOf: (id: number) => number = () => 1, sizeOf: (id: number) => number = () => 1) {
    const { tree, items } = indexTree(roots, (n) => n.under);
    const scale = new Float64Array(items.length);
    items.forEach((_, k) => {
      const p = tree.parent[k];
      scale[k] = p < 0 ? 1 : scale[p] * sizeOf(items[p].id);
    });
    const rows = Float64Array.from(items, (n, k) => rowsOf(n.id) * scale[k]);
    const top = layOut(tree, rows, scale);
    const by = new Map(items.map((n, k) => [n.id, { top: top[k], rows: rows[k], end: top[k] + rows[k] }]));
    return { tree, top, rows, at: (id: number) => by.get(id) as { top: number; rows: number; end: number } };
  }

  it('stands each item in the middle of the items under it, and no item of a column over another, in any tree', () => {
    // Thirty trees drawn at random, the same each run: up to five roots, up to three items under an item at four
    // levels, a third of the items with a second row for their work.
    let seed = 7;
    const random = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
    let id = 0;
    const grow = (depth: number): Node =>
      node((id += 1), depth < 4 ? Array.from({ length: Math.floor(random() * 4) }, () => grow(depth + 1)) : []);
    let parents = 0;
    for (let n = 0; n < 30; n += 1) {
      const roots = Array.from({ length: 1 + Math.floor(random() * 5) }, () => grow(0));
      const worked = new Set(Array.from({ length: id }, (_, i) => i + 1).filter(() => random() < 0.3));
      const { tree, top, rows } = laid(roots, (i) => (worked.has(i) ? 1.55 : 1));
      for (let k = 0; k < top.length; k += 1) {
        expect(top[k]).toBeGreaterThanOrEqual(0);
        const kids = tree.children[k];
        if (kids.length === 0) continue;
        parents += 1;
        const [first, last] = [kids[0], kids[kids.length - 1]];
        expect(top[k] + rows[k] / 2).toBeCloseTo((top[first] + top[last] + rows[last]) / 2, 9);
      }
      for (const column of tree.columns)
        for (let r = 1; r < column.length; r += 1)
          expect(top[column[r]]).toBeGreaterThanOrEqual(top[column[r - 1]] + rows[column[r - 1]] - 1e-9);
    }
    expect(parents).toBeGreaterThan(100);
  });

  it('sets two groups of a column apart only where either holds more than one item, and stands a lone item level with the item it hangs from', () => {
    // Three roots, one item under each: every item level with its root, the column as close as the roots.
    const lone = laid([node(1, [node(11)]), node(2, [node(21)]), node(3, [node(31)])]);
    expect([1, 2, 3, 11, 21, 31].map((i) => lone.at(i).top)).toEqual([0, 1, 2, 0, 1, 2]);
    // Two items under the middle root: room above them and below them.
    const pair = laid([node(1, [node(11)]), node(2, [node(21), node(22)]), node(3, [node(31)])]);
    expect(pair.at(21).top).toBeGreaterThan(pair.at(11).end);
    expect(pair.at(31).top).toBeGreaterThan(pair.at(22).end);
    // No room between two items of one group.
    expect(pair.at(22).top).toBe(pair.at(21).end);
  });

  it('draws a group a fold turns, and the room that sets it apart, at the size the fold draws it', () => {
    // Under 1, three items; under 2, two, which a fold turns to `size` of their whole size.
    const turning = (size: number) =>
      laid([node(1, [node(11), node(12), node(13)]), node(2, [node(21), node(22)])], () => 1, (i) => (i === 2 ? size : 1));
    const whole = turning(1);
    const room = (l: ReturnType<typeof turning>) => l.at(21).top - l.at(13).end;
    const span = (l: ReturnType<typeof turning>) => l.at(22).end - l.at(21).top;
    for (const size of [0.5, 0.25, 0.05]) {
      const part = turning(size);
      expect(room(part)).toBeCloseTo(room(whole) * size, 9);
      expect(span(part)).toBeCloseTo(span(whole) * size, 9);
    }
  });
});

describe('the items near the pointer', () => {
  it('are drawn at their full size at the pointer, smaller further from it, and at the size of the tree from a few rows or a column away', () => {
    for (const zoom of [0.2, 0.5, 0.9]) {
      expect(lensSize(0, 0, zoom)).toBe(1);
      expect(lensSize(50, 0, zoom)).toBe(zoom);
      expect(lensSize(0, 3, zoom)).toBe(zoom);
      for (let rows = 0; rows < 10; rows += 0.5) {
        expect(lensSize(rows + 0.5, 0, zoom)).toBeLessThanOrEqual(lensSize(rows, 0, zoom));
        expect(lensSize(rows, 0.5, zoom)).toBeLessThanOrEqual(lensSize(rows, 0, zoom));
        expect(lensSize(rows, 0, zoom)).toBeGreaterThanOrEqual(zoom);
      }
    }
    // At its full size, the tree draws nothing larger.
    expect(lensSize(1, 0.2, 1)).toBe(1);
  });
});

describe("an item's brightness", () => {
  it('goes up at once to full where the pointer comes onto it, and down over frames where the light leaves it', () => {
    expect(dimmed(DIM, 1, 16)).toBe(1);
    const down = dimmed(1, DIM, 16);
    expect(down).toBe(eased(1, DIM, 16));
    expect(down).toBeLessThan(1);
    expect(down).toBeGreaterThan(DIM);
  });
});

describe("a running cell's beat on its card", () => {
  // The page's build writes the token's time in seconds (`.75s`), the page's source in milliseconds: the card's mark
  // beats once in 750 ms either way, as the works page's marks do, never once in 0.75 ms.
  it.each(['750ms', '.75s', '0.75s'])('is 750 ms when the styles write %s', (written) => {
    document.documentElement.style.setProperty('--pulse', written);
    try {
      expect(beatOf().ms).toBe(750);
    } finally {
      document.documentElement.style.removeProperty('--pulse');
    }
  });
});
