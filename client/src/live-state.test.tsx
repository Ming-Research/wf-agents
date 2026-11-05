// What the page says a work is doing, one reading for every place: the list of messages, the head of a work's group, the
// halo and the cells' rings, and the list of a work's rounds. While the engine stops, a work that was to go on is not
// running, whatever its row reads; between two steps of the engine a round that runs does not read as left open.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { CompiledGraph, RoundSummary, Status, WorkSummary } from './api';
import { course, liveState } from './components';
import { RoundsList, leftOpen } from './pages/WorkChart';
import { workMark } from './pages/Messages';
import { standingWords } from './pages/System';
import { WorkState as HeadState } from './pages/TalkHead';

// What the page has read, which each test sets: the engine's status and the works.
const read = vi.hoisted(() => ({ status: null as Status | null, works: [] as WorkSummary[] }));

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => read.status,
  useActivity: () => [],
  useShared: (_key: string, path: string) => (path === '/api/v1/works' ? read.works : []),
}));

const status = (extra: Partial<Status> = {}): Status =>
  ({
    paused: false,
    keep_warm: true,
    started: [],
    releasing: [],
    waiting_for_slot: [],
    stopping: [],
    engine_stopping: false,
    architects_started: [],
    running: [],
    stepping: [],
    last_step: null,
    slots: 2,
    slots_used: 0,
    flow_version: 3,
    config_commit: 'c',
    repos: {},
    started_at: 1,
    version: 'v',
    halt: null,
    ...extra,
  }) as Status;

/// A work whose row reads `state`, with the round its last step left open when it reads running.
const work = (id: number, state: WorkSummary['state'], extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: id,
  title: `Item ${id}`,
  project: 'p',
  graph: 'engineer',
  flow_version: 3,
  moved_from: null,
  at: 'D1',
  at_title: 'Design',
  stage: 'design',
  state,
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: 1,
  last_at: 2,
  ran_ms: 0,
  time: { from: 0, until: 0, model_ms: 0, tool_ms: 0, engine_ms: 0, bar: '' },
  rounds: 3,
  sent_back: 0,
  failed: 0,
  running: state === 'running' ? { n: 3, cell: 'D1', started_at: 2, tool: false, line: 'engineer' } : null,
  cost_usd: 0,
  open_asks: 0,
  open_design_cards: 0,
  ...extra,
});

const round = (n: number, extra: Partial<RoundSummary> = {}): RoundSummary => ({
  work: 7,
  n,
  cell: 'D1',
  line: 'engineer',
  outcome: 'running',
  transition: null,
  why: null,
  set_aside: null,
  started_at: 10,
  ended_at: null,
  model: null,
  cost_usd: null,
  compactions: 0,
  tool_runs: 0,
  progress: null,
  not_asked: null,
  tokens: null,
  session: null,
  new_session: null,
  ...extra,
});

beforeEach(() => {
  read.status = null;
  read.works = [];
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('a work while the engine stops', () => {
  it('is running only where a step of it is in flight: the works that were to go on wait for the engine to start again', () => {
    const goesOn = work(7, 'ready');
    const inFlight = work(8, 'running');
    const going = status({ running: [8] });
    // As the engine runs, a work with its next step due is running, whether or not a step of it is in flight.
    expect(liveState(goesOn, going)).toBe('running');
    expect(liveState(inFlight, going)).toBe('running');
    // Once the engine has seen its stop file, it takes no next step: only the step in flight runs.
    const stopping = status({ running: [8], engine_stopping: true });
    expect(liveState(goesOn, stopping)).toBe('ready');
    expect(course(goesOn, stopping).working).toBeNull();
    expect(liveState(inFlight, stopping)).toBe('running');
    // A work the row reads as running that has no step in flight is ready, as it is while the engine is out of reach.
    expect(liveState(work(9, 'running'), status({ engine_stopping: true }))).toBe('ready');
    // The tool of a round that is under way works on after the engine's step of it ended.
    const tool = work(10, 'running', { running: { n: 3, cell: 'X1', started_at: 2, tool: true, line: null } });
    expect(liveState(tool, stopping)).toBe('running');
  });

  it('shows one running among eight recorded as going on when the engine stops with a step of one in flight', () => {
    const rows = [1, 2, 3, 4, 5, 6, 7, 8].map((id) => work(id, id % 2 === 0 ? 'running' : 'ready'));
    const running = (s: Status) => rows.filter((w) => liveState(w, s) === 'running').map((w) => w.id);
    expect(running(status({ running: [2] }))).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
    expect(running(status({ running: [2], engine_stopping: true }))).toEqual([2]);
  });
});

describe('the icon and the title of a work in the list of messages', () => {
  it('both say running for a work the engine has a step of in flight, though its row still reads ready', () => {
    const w = work(7, 'ready');
    read.status = status({ running: [7] });
    read.works = [w];
    // The icon of its conversation, the head of its group and the System view's words come from the one reading.
    expect(workMark(7, read.works, read.status)).toBe('running');
    expect(liveState(w, read.status)).toBe('running');
    expect(standingWords(w, read.status)).toBe('running');
    expect(renderToStaticMarkup(<HeadState w={w} status={read.status} />)).toContain('running');
    // Not in flight, with the engine halted: ready on all of them.
    const halted = status({ halt: { kind: 'offline', until: 1, why: 'no network' } });
    expect(workMark(7, read.works, halted)).toBe('ready');
    expect(renderToStaticMarkup(<HeadState w={w} status={halted} />)).not.toContain('running');
  });
});

describe("a work's rounds between two steps of the engine", () => {
  const g = { cells: [] } as unknown as CompiledGraph;
  const list = (rounds: RoundSummary[]) =>
    renderToStaticMarkup(<RoundsList rounds={rounds} g={g} selected={null} onRound={() => {}} />);

  it('keeps a round that runs from reading as left open while its work goes on, and says it where nothing takes it up', () => {
    const open = round(3);
    const w = work(7, 'running');
    // The engine's status names no step of the work for a moment: its next step is due and takes the round up.
    const between = status({ running: [] });
    expect(course(w, between).working).toBe('D1');
    expect(leftOpen(open, between, [w])).toBe(false);
    // A step of it in flight, the same.
    expect(leftOpen(open, status({ running: [7] }), [w])).toBe(false);
    // Nothing takes it up: the engine halted, every work paused, the work paused by the owner, or the engine stopping.
    for (const s of [
      status({ halt: { kind: 'offline', until: 1, why: 'no network' } }),
      status({ paused: true }),
      status({ engine_stopping: true }),
    ]) {
      expect(leftOpen(open, s, [w])).toBe(true);
    }
    expect(leftOpen(open, status(), [work(7, 'running', { paused: 'you paused it', paused_by: 'owner' })])).toBe(true);
    // A round that ended is no round left open; with the engine out of reach nothing can be told.
    expect(leftOpen(round(3, { outcome: 'took', ended_at: 11 }), status({ paused: true }), [w])).toBe(false);
    expect(leftOpen(open, null, [w])).toBe(false);
    // The works not read yet: the status alone says whether a step of the work is in flight.
    expect(leftOpen(open, status({ running: [7] }), undefined)).toBe(false);
    expect(leftOpen(open, status({ running: [] }), undefined)).toBe(true);
  });

  it('draws the row of the round running as running between two steps, and as left open where nothing takes it up', () => {
    read.works = [work(7, 'running')];
    read.status = status({ running: [] });
    expect(list([round(3)])).not.toContain('left open');
    read.status = status({ halt: { kind: 'offline', until: 1, why: 'no network' } });
    expect(list([round(3)])).toContain('left open');
  });
});
