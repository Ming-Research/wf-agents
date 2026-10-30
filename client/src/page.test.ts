import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type {
  CompiledCell,
  CompiledGraph,
  CompiledRow,
  DesignCard,
  EngineNames,
  Form,
  GraphView,
  JsonValue,
  Conversation,
  ConversationDetail,
  ConversationEntry,
  Forwarded,
  RoundSummary,
  RoundTokens,
  Status,
  Use,
  WorkSummary,
} from './api';
import { controls, course, heldName, heldWords } from './components';
import { held, runningWords, stepWhyNot, waitOf } from './course';
import {
  cellTab,
  cellTabs,
  cellWhere,
  counterFirst,
  enteredFrom,
  fallbackWords,
  leavesTo,
  numbered,
  ownMoves,
  telling,
} from './flow/CellPanel';
import { inputName } from './flow/CellGiven';
import { fieldActions, shownNote, titleOf } from './flow/Returns';
import { conditionsWords, conditionWords, targetWords, valuesWords } from './flow/words';
import { cost, duration } from './format';
import { pickedOnChart } from './flow/FlowScreen';
import {
  chartFonts,
  cornerWidth,
  EFFORTS,
  effortOf,
  effortsOf,
  effortWidth,
  FlowSvg,
  labelSize,
  lineKind,
  litCell,
  type Marks,
  sessionWidth,
  sizes,
  startsAnew,
  styleOf,
} from './graph/flowDraw';
import { addHops, type Box, type Laid, lengthenLast, onLine, pathOf, type Route as Routed } from './graph/flowLayout';
import { moveWords, pickAt, type Under } from './graph/FlowChart';
import { centredAt, draggedBy, LEAST, mapGeom, mapOf, mapShown, viewFrame } from './graph/Minimap';
import {
  ASKING,
  chart,
  HALO_HOLD,
  HALO_STEP,
  haloAt,
  letterOf,
  MAIN,
  mainPath,
  moves,
  overlays,
  partsOf,
  related,
  roundArrows,
  taken,
  ways,
} from './graph/flowModel';
import { layStores, linkPath, storeLinks, storeUses } from './graph/stores';
import { named } from './live';
import { parse as parseMarkdown } from './markdown';
import { currentCard, firstSentence, pickApproach, verdictLine, verdictReady, type Verdict } from './pages/Decide';
import {
  closedWhy,
  conversationTitle,
  forwardMenu,
  forwardNewChoices,
  forwardParts,
  forwardWords,
  heldCards,
  latestAt,
  listParts,
  mention,
  mentions,
  newlyWaiting,
  party,
  pendingWords,
  sayTo,
  sayToTicked,
  streamOf,
  totals,
  voiceOf,
  waitingByKey,
} from './conversation';
import { addressesByNumber, atPath, modelName } from './pages/RoundView';
import { roundsShown } from './pages/Work';
import { catalog, type Key, t, tn } from './messages';
import { href, parse, type Route } from './route';
import { chars, messageParts, placeSchema, shape, shareWords, spent, tokens } from './round';
import { settingsChange, settingsProblems } from './pages/Settings';
import { tiersAllowed } from './pages/TalkHead';
import { spendTable } from './spend';
import { axisTicks, gapsOf, usageOf } from './usage';

describe('the message catalog', () => {
  it('has every text, none empty, all in English', () => {
    for (const [key, text] of Object.entries(catalog)) {
      expect(text.trim(), key).not.toBe('');
      // No character of a CJK script, nor full-width punctuation.
      expect(/[\u{3000}-\u{303f}\u{3040}-\u{30ff}\u{3400}-\u{9fff}\u{ff00}-\u{ffef}]/u.test(text), key).toBe(false);
    }
    expect(t('slots', { used: 1, total: 2 })).toBe('Slots 1/2');
  });

  // One rule says every count: a text that counts in the plural has a `_one` form for one, and the page says it through
  // tn, which picks that form, never through t alone.
  it('says every count right for one, through tn', () => {
    const keys = Object.keys(catalog) as Key[];
    for (const k of keys.filter((x) => x.endsWith('_one'))) {
      const base = k.slice(0, -'_one'.length) as Key;
      expect(keys, k).toContain(base);
      expect(catalog[base], base).toContain('{n}');
      expect(catalog[k], k).not.toContain('{n}');
    }
    // A count before a plural word: "{n} matters left", "{n} keep-warm pings", "reported {n} times". The two whose
    // words read right for one have none.
    const plural = /\{n\} (?:[\w-]+ )?(?:[\w-]+s|times)\b/;
    const right = ['from_facts_back', 'given_new_n'];
    for (const k of keys) {
      if (!k.endsWith('_one') && !right.includes(k) && plural.test(catalog[k])) expect(keys, k).toContain(`${k}_one`);
    }
    const sources = import.meta.glob<string>(
      ['./**/*.ts', './**/*.tsx', '!./**/*.test.ts', '!./**/*.test.tsx', '!./messages.ts', '!./api.ts'],
      { query: '?raw', import: 'default', eager: true },
    );
    for (const [file, text] of Object.entries(sources)) {
      for (const line of text.split('\n')) {
        for (const m of line.matchAll(/'(\w+)'/g)) {
          expect(m[1].endsWith('_one') && m[1] in catalog, `${file} names ${m[1]}`).toBe(false);
          if (`${m[1]}_one` in catalog) expect(line, `${file} says ${m[1]}`).toMatch(/\btn\(/);
        }
      }
    }
    expect(tn('talk_queue_left', 1)).toBe('1 matter left');
    expect(tn('talk_queue_left', 3)).toBe('3 matters left');
    expect(tn('observation_reported', 1)).toBe('reported once');
    expect(tn('observation_reported', 2)).toBe('reported 2 times');
    expect(tn('stage_fixes', 1)).toBe('1 fix');
    expect(tn('n_tokens', 1, { n: '1' })).toBe('1 token');
    expect(tn('n_tokens', 12_000, { n: '12K' })).toBe('12K tokens');
    expect(tn('items_deps_title', 1, { done: 0 })).toBe('Done: 0 of the 1 item it depends on');
    expect(tn('nav_unread', 1)).toBe('1 new since you last read');
  });
});

describe('the addresses', () => {
  it('read back what they write', () => {
    const flow = { page: 'flow' as const, version: null, graph: null, cell: null, project: null, ref: null, from: null };
    const work = { page: 'work' as const, id: 3, round: null, cell: null, tab: 'rounds' as const, only: null };
    const routes: Route[] = [
      { page: 'works' },
      { ...work, round: 240 },
      { ...work, cell: 'D2' },
      { ...work, tab: 'cards' },
      { ...work, tab: 'helpers', round: 7 },
      { ...work, only: 'failed' },
      { page: 'items' },
      { page: 'item', id: 5 },
      { page: 'messages', open: null },
      { page: 'messages', open: 'architects:p|q' },
      { page: 'system', period: 'week' },
      { page: 'system', period: '4w' },
      { page: 'system', period: 'all' },
      { ...flow, version: 1, graph: 'architect', cell: 'X1' },
      flow,
      { ...flow, version: 2, from: 1 },
      { ...flow, project: 'p', ref: 'wf-agents/12-x', from: 1, graph: 'engineer', cell: 'I1' },
    ];
    for (const r of routes) {
      const at = href(r);
      const i = at.indexOf('?');
      expect(i < 0 ? parse(at, '') : parse(at.slice(0, i), at.slice(i))).toEqual(r);
    }
    // A round is named by its work and its number in the work; a name of another work's round opens none.
    expect(href({ ...work, round: 240 })).toBe('/works/3?round=R3-240');
    expect(parse('/works/3', '?round=R4-240')).toEqual(work);
    expect(parse('/works/3', '?round=240')).toEqual(work);
    expect(parse('/', '')).toEqual({ page: 'works' });
    expect(parse('/works/x', '')).toEqual({ page: 'missing' });
    // A tab or a choice of rounds the page does not know is the work's rounds, all of them.
    expect(parse('/works/3', '?tab=x&only=y')).toEqual(work);
    expect(href(work)).toBe('/works/3');
    // The System view counts the week under way unless the address names another period; the Spend view it took the
    // place of leads to it.
    expect(href({ page: 'system', period: 'week' })).toBe('/system');
    expect(parse('/system', '?period=x')).toEqual({ page: 'system', period: 'week' });
    expect(parse('/spend', '')).toEqual({ page: 'system', period: 'week' });
  });
});

describe('how a work goes on', () => {
  const work = (extra: Partial<WorkSummary>): WorkSummary => ({
    id: 5,
    item: 1,
    title: 't',
    project: 'p',
    graph: 'engineer',
    flow_version: 2,
    at: 'A3',
    at_title: 'Owner',
    stage: null,
    state: 'ready',
    paused: null,
    paused_by: null,
    ended: null,
    waits: null,
    branch: null,
    pr: null,
    started_at: null,
    last_at: null,
    rounds: 0,
    sent_back: 0,
    failed: 0,
    running: null,
    cost_usd: 0,
    open_asks: 0,
    open_design_cards: 0,
    ...extra,
  });
  const status = (extra: Partial<Status>): Status => ({
    paused: false,
    keep_warm: true,
    started: [],
    releasing: [],
    architects_started: [],
    running: null,
    stepping: null,
    last_step: null,
    slots: 1,
    slots_used: 0,
    flow_version: 2,
    config_commit: 'c',
    repos: {},
    started_at: 0,
    version: 'v',
    halt: null,
    ...extra,
  });

  it('keeps what holds a work back apart from what it waits for, and offers what does something', () => {
    // Paused by the owner and waiting for an answer: both; Start, and Step greyed while it waits.
    const both = course(
      work({ state: 'paused', paused: 'the owner paused it', paused_by: 'owner', waits: { kind: 'answer', ask: 6 } }),
      status({}),
    );
    expect([both.held, both.waits]).toEqual(['you', { kind: 'answer', ask: 6 }]);
    expect(controls(both, status({}))).toEqual({ pause: false, start: true, step: false, canStep: true });
    expect(stepWhyNot(both, controls(both, status({})))).toBe('waiting for your answer to ask 6');
    expect(held(both, work({ paused: 'the owner paused it', paused_by: 'owner' }))).toBe('stopped (you paused it)');
    expect(waitOf(both, work({}), status({}))?.tag).toBe('waiting for you');
    // Every work paused: stopped by the global pause, said so; Start and Step offered, and no Pause beside Start.
    const all = course(work({}), status({ paused: true }));
    // Beside its cell: waiting for Step while Step can run a round of it, paused while another step is under way.
    expect(waitOf(all, work({}), status({ paused: true }))).toEqual({
      tag: 'waiting for Step',
      why: 'Every work is paused: Start runs this one on whatever the global state, and Step runs one round of it.',
    });
    expect(waitOf(all, work({}), status({ paused: true, stepping: 3 }))?.tag).toBe('paused');
    expect([all.held, heldName(all).name]).toEqual(['all', 'stopped (global pause)']);
    expect(controls(all, status({ paused: true }))).toEqual({ pause: false, start: true, step: true, canStep: true });
    expect(stepWhyNot(all, controls(all, status({ paused: true })))).toBe(null);
    expect(held(all, work({}))).toBe(
      'Every work is paused: Start runs this one on whatever the global state, and Step runs one round of it.',
    );
    // Started by the owner while every work is paused: it goes on, said so; Pause, and Step greyed till it is paused.
    const started = course(work({}), status({ paused: true, started: [5], running: 5 }));
    expect([started.held, started.started, started.running]).toEqual([null, true, 'goes_on']);
    expect(runningWords(started)).toBe('running (you started it)');
    expect(controls(started, status({ paused: true, started: [5] }))).toEqual({
      pause: true,
      start: false,
      step: false,
      canStep: true,
    });
    expect(stepWhyNot(started, controls(started, status({ paused: true, started: [5] })))).toBe(
      'It goes on: Step runs a round of it only while it is paused.',
    );
    // Started while paused by itself, its pause waiting for a free slot: said so; its Pause takes the Start back, and
    // it is not stepped, as it goes on.
    const slot = course(
      work({ paused: 'the owner paused it', paused_by: 'owner' }),
      status({ paused: true, started: [5], releasing: [5] }),
    );
    expect([slot.held, heldName(slot).name]).toEqual(['you', 'started, waiting for a free slot']);
    expect([controls(slot, status({})).pause, controls(slot, status({})).start]).toEqual([true, false]);
    expect(controls(slot, status({})).step).toBe(false);
    // A work of an architect goes on while the owner started that project's architect, and not for another's.
    const arch = course(
      work({ graph: 'architect', item: null, project: 'p' }),
      status({ paused: true, architects_started: ['p'] }),
    );
    expect([arch.held, arch.started]).toEqual([null, true]);
    const other = course(
      work({ graph: 'architect', item: null, project: 'q' }),
      status({ paused: true, architects_started: ['p'] }),
    );
    expect(other.started).toBe(false);
    // Going on by itself, its round running: Pause only; once paused, it pauses after the round, Step greyed till then.
    const on = course(work({}), status({ running: 5 }));
    expect([on.running, controls(on, status({ running: 5 })).pause]).toEqual(['goes_on', true]);
    expect(waitOf(on, work({}), status({ running: 5 }))).toBeNull();
    // Going on by itself between two rounds, or waiting for other items or the architect, says so or nothing.
    expect(waitOf(course(work({}), status({})), work({}), status({}))).toBeNull();
    const items = course(work({ waits: { kind: 'items', items: [3] } }), status({}));
    expect(waitOf(items, work({}), status({}))?.tag).toBe('waiting for other items');
    const reply = course(work({ waits: { kind: 'architect', matter: 2 } }), status({}));
    expect(waitOf(reply, work({}), status({}))?.tag).toBe('waiting for the architect');
    const pausing = course(work({ paused: 'x', paused_by: 'owner' }), status({ running: 5 }));
    expect(pausing.running).toBe('pausing');
    expect(stepWhyNot(pausing, controls(pausing, status({ running: 5 })))).toBe('pausing after this round');
    expect(course(work({ paused: 'x', paused_by: 'owner' }), status({ running: 5, stepping: 5 })).running).toBe('step');
    // Stopped by the engine on an older flow version: Start and Step, which wait for the round under way.
    const stopped = course(work({ paused: 'cell implement: no', paused_by: 'engine', flow_version: 1 }), status({}));
    expect([stopped.held, stopped.outdated]).toEqual(['engine', true]);
    const why = waitOf(stopped, work({ paused: 'cell implement: no', paused_by: 'engine' }), status({}));
    expect(why).toEqual({ tag: 'stopped by the engine', why: 'stopped (by the engine): cell implement: no' });
    expect(controls(stopped, status({ stepping: 3 }))).toEqual({ pause: false, start: true, step: true, canStep: false });
    expect(stepWhyNot(stopped, controls(stopped, status({ stepping: 3 })))).toBe(
      'Another round you asked for is under way, or something stops every work: Step waits until it ends.',
    );
    // Stopped by the architect: said so, with its reason, not as a problem; Start all leaves it, its Start lets it go on.
    const architect = course(work({ paused: 'it waits for the parser', paused_by: 'architect' }), status({ paused: true }));
    expect([architect.held, heldName(architect)]).toEqual([
      'architect',
      { name: 'stopped (by the architect)', problem: false },
    ]);
    expect(heldWords(architect, 'it waits for the parser')).toBe('stopped (by the architect): it waits for the parser');
    expect(controls(architect, status({ paused: true })).start).toBe(true);
    // An ended work goes on no more.
    expect(controls(course(work({ state: 'ended', ended: 'x' }), status({})), status({})).pause).toBe(false);
  });
});

describe('the formats', () => {
  it('write durations in their two largest units, and costs to the cent', () => {
    expect(duration(41_000)).toBe('41s');
    expect(duration(187_000)).toBe('3m 07s');
    expect(duration(7_500_000)).toBe('2h 05m');
    expect(cost(1.234)).toBe('$1.23');
    expect(cost(null)).toBe('');
  });
});

describe('the flow in words', () => {
  it('writes conditions and targets as words, names in code', () => {
    const known = new Set(['split']);
    expect(conditionWords(['own'], known)).toBe("the return is this cell's own answer");
    expect(conditionWords(['has', 'tools'], known)).toBe('the return has `tools`');
    expect(conditionWords(['is', 'ask.to', ['advisor']], known)).toBe('`ask.to` is `advisor`');
    expect(conditionWords(['inp', 'split', ['all_split']], known)).toBe("the engine's value `split` is `all_split`");
    expect(conditionWords(['inp', 'stage', ['research']], known)).toBe('the recorded value `stage` is `research`');
    expect(conditionsWords([['none', 'failed.kind', ['a', 'b', 'c']], ['all']], known)).toBe(
      'no `failed.kind` is `a`, `b` or `c`, and always',
    );
    expect(valuesWords(['x', 'y'])).toBe('`x` or `y`');
    expect(targetWords('@caller')).toBe('back to the cell that called');
    expect(targetWords('@self')).toBe('itself');
    expect(targetWords('I1')).toBeNull();
  });
});

describe('events', () => {
  it('name what to read again', () => {
    expect(named({ kind: 'round', work: 2, round: 9 })).toEqual([
      'works',
      'work:2',
      'round:2-9',
      'items',
      'item:*',
      'messages',
      'system',
    ]);
    expect(named({ kind: 'item', item: null })).toEqual(['items', 'item:*', 'works', 'system']);
    expect(named({ kind: 'ready' })).toEqual([]);
    expect(named({ kind: 'ask', ask: 4, work: 2 })).toEqual(['works', 'work:2', 'item:*', 'messages', 'system']);
    expect(named({ kind: 'owner_message', message: 7, work: 2 })).toEqual(['messages', 'work:2', 'system']);
  });
});

function row(name: string, to: string, extra: Partial<CompiledRow> = {}): CompiledRow {
  return {
    name,
    if: [['all']],
    to,
    counts: null,
    waits: false,
    pauses: false,
    interject: false,
    main: false,
    common: false,
    note: null,
    ...extra,
  };
}

function cell(id: string, rows: CompiledRow[], extra: Partial<CompiledCell> = {}): CompiledCell {
  return {
    id,
    kind: 'agent',
    gate: false,
    title: `cell ${id}`,
    step: null,
    needs: null,
    line: 'engineer',
    stage: 'build',
    inputs: [],
    prompt_by: null,
    schema: null,
    rows,
    ...extra,
  };
}

function round(n: number, cellId: string | null, outcome: RoundSummary['outcome'], transition: string | null): RoundSummary {
  return {
    n,
    cell: cellId,
    line: null,
    outcome,
    transition,
    why: null,
    started_at: n,
    ended_at: outcome === 'running' ? null : n + 1,
    model: null,
    cost_usd: null,
    compactions: 0,
    tool_runs: 0,
    progress: null,
    not_asked: null,
    tokens: null,
    session: null,
    new_session: null,
  };
}

// A start; four agent cells, each with the moves of its own its forms give it: every one runs tools and pauses, all
// but `d` ask `q`, and only `a` searches, which only the engineer line may; `b` and `c` also go to `q` by answers of
// their own; `a` and `c` lead into `b` through the same actions, and `d` is reached by no row; an engine step that
// decides by a value of the engine's and waits through an action; the tools, the search, the pause, an asking part
// that answers back, and the end.
// A form as the compiled flow writes it: the field and the lines left out when it has none.
const form = (field: string | null, lines: string[] | null = null): Form => ({
  say: '',
  ...(field !== null ? { field } : {}),
  ...(lines !== null ? { lines } : {}),
});
const forms = [form(null), form('ask'), form('tools'), form('search', ['engineer']), form('pause')];
const without = (...fields: string[]) => forms.filter((f) => f.field == null || !fields.includes(f.field));
const move = (name: string, to: string, extra: Partial<CompiledRow> = {}) =>
  row(name, to, { if: [['has', name]], common: true, ...extra });
const asks = row('ask_q', 'q', { if: [['is', 'ask.to', ['q']]], common: true });
const sentBack = row('sent_back', '@self', { if: [['sent_back']], common: true, fallback: true });
const told = { at: 'tell_owner', view: null, do: 'tell_owner' };
const graph: CompiledGraph = {
  name: 'engineer',
  start: 's',
  end: 'z',
  within: 'outcome',
  asking: ['q', 'r'],
  receive: null,
  counters: [],
  rules: [{ name: 'tree', desc: 'How the tree is written', text: ['Write it so.'] }],
  interject: { name: 'interjection', desc: 'What is said', text: [], from: [] },
  rounds: {
    desc: '',
    sentBack: { desc: 'Sent back to be written again.', pauseAfter: 3, pause: 'Paused after three.' },
    failed: { desc: '', kinds: [] },
    record: [],
    environment: [],
  },
  cells: [
    cell('s', [row('started', 'a', { main: true })], { kind: 'engine', line: null, stage: 'start' }),
    cell(
      'a',
      [
        asks,
        row('done', 'b', { if: [['own']], main: true, do: ['lock', { at: 'card', do: 'post' }] }),
        row('again', 'b'),
        row('other', 'c'),
        move('tools', 't', { main: true }),
        move('search', 'x', { main: true }),
        move('pause', 'p'),
        sentBack,
      ],
      { forms, always: [told], rules: [{ name: 'tree', shared: false }, { name: 'general', shared: true }] },
    ),
    cell(
      'b',
      [
        asks,
        row('done', 'z', { main: true }),
        row('back', 'a'),
        move('tools', 't'),
        move('pause', 'p'),
        row('unsure', 'q'),
        sentBack,
      ],
      { line: 'review', forms: without('search'), always: [told, { at: 'card', view: null, do: 'update_card' }] },
    ),
    cell(
      'c',
      [asks, row('ok', 'b', { do: ['lock', 'post'] }), row('unsure', 'q'), move('tools', 't'), move('pause', 'p'), sentBack],
      { line: 'observer', forms: without('search'), always: [told] },
    ),
    cell('d', [row('ok', 'm'), move('tools', 't'), move('pause', 'p'), sentBack], {
      forms: without('search', 'ask'),
      always: [told],
    }),
    cell(
      'm',
      [
        row('wait', 'm', { if: [['inp', 'gates', ['waiting']]], waits: true, do: ['notify'] }),
        row('clear', 'z', { if: [['inp', 'gates', ['clear']]], main: true }),
      ],
      { kind: 'engine', line: null, inputs: [{ id: null, what: 'the cards', from: [{ db: 'design_cards', by: 'item' }], maybe: false }] },
    ),
    cell('t', [row('ran', '@caller'), row('failed', 'r')], {
      kind: 'tool',
      line: null,
      stage: null,
      always: [{ at: 'results', view: null, do: 'record' }],
    }),
    cell('x', [row('found', '@caller')], { kind: 'tool', line: null, stage: null }),
    cell('p', [row('started', '@caller', { waits: true })], { kind: 'engine', line: null, stage: null }),
    cell('q', [row('answered', '@asker')], { kind: 'engine', line: null, stage: null }),
    cell('r', [row('back', '@stopped'), row('redo', 'a')], { kind: 'engine', line: null, stage: null }),
    cell('z', [row('interjection', 'a', { interject: true })], { kind: 'engine', line: null, stage: 'done' }),
  ],
};

// The cells a row's target can be: whoever has a row to the cell, for one back to whoever called or asked.
const callers = (id: string) => graph.cells.filter((x) => x.rows.some((r) => r.to === id)).map((x) => x.id);
const resolve = (c: CompiledCell, to: string): string[] => {
  if (to === '@self') return [c.id];
  if (to === '@caller' || to === '@asker') return callers(c.id);
  if (to === '@stopped') return ['a'];
  return [to];
};
const view: GraphView = {
  lines: [],
  name: 'engineer',
  common_format: '',
  cells: graph.cells.map((c) => ({
    id: c.id,
    targets: c.rows.map((r) => resolve(c, r.to)),
    entered_from: graph.cells.flatMap((x) =>
      x.rows.flatMap((r, i) => (resolve(x, r.to).includes(c.id) ? [{ cell: x.id, row: i }] : [])),
    ),
    inputs: [],
    returns: { within: null, forms: [], every: [], used_by: [] },
  })),
};

const engine: EngineNames = {
  steps: {},
  actions: { lock: 'Locks it.', post: 'Posts it.', notify: 'Tells you.' },
  checks: {},
  inputs: {},
  values: { gates: 'Whether the gates are clear.' },
  needs: {},
  stores: {
    cards: { title: 'Design cards', desc: 'The cards', layer: 'round', relations: ['design_cards'] },
    queue: { title: "The architect's queue", desc: 'What waits', layer: 'project' },
    acceptance: { title: 'Locked acceptance', desc: 'What is locked', layer: 'item' },
    merged: { title: 'Merge record', desc: 'What merged', layer: 'item', computed: true },
  },
  writes: { lock: ['acceptance'], post: ['cards', 'queue'], notify: [], update_card: ['cards'], record: [] },
  reads: { gates: { says: 'dependencies merged', stores: ['merged'] } },
};

describe('the chart of a flow', () => {
  const c = chart(graph, view, engine);
  const node = (id: string) => c.nodes.find((n) => n.id === id);
  const edge = (id: string) => c.edges.find((e) => e.id === id);

  it('draws the moves of a cell of its own by how many of the cells that could make them do', () => {
    const tiers = Object.fromEntries(moves(graph).map((m) => [m.row, [m.tier, m.cells.join(''), m.could.join('')]]));
    // Every cell runs tools and pauses: the legend says so. All but one ask: the one that does not is marked.
    expect(tiers).toEqual({
      ask_q: ['marked', 'abc', 'abcd'],
      tools: ['legend', 'abcd', 'abcd'],
      search: ['lines', 'a', 'ad'],
      pause: ['legend', 'abcd', 'abcd'],
    });
    expect(c.notes.get('d')?.lacks.map((m) => m.row)).toEqual(['ask_q']);
    // A search, which few cells make, is a line; tools, pauses and asks are not, nor the engine's fallback.
    expect(edge('c:a->c:x')).toMatchObject({ label: 'search' });
    // A move is on no main path, on the chart or in the panel, though its row says it is; a cell's own answer is.
    expect(edge('c:a->c:x')?.main).toBe(false);
    expect(mainPath(graph.cells[1].rows[5])).toBe(false);
    expect(mainPath(graph.cells[1].rows[1])).toBe(true);
    expect(c.edges.filter((e) => ['c:t', 'c:p', 'o:q:a'].includes(e.target))).toEqual([]);
    expect(c.edges.some((e) => e.label.includes('sent_back'))).toBe(false);
    expect(moveWords(moves(graph)[0])).toBe('ask q → q: every agent cell, but d, each marked');
    expect(moveWords(moves(graph)[2])).toBe('search → x: a line from each cell that does (1 of the 2 that could)');
  });

  it('puts the asking part apart, a cell only moves in the legend lead to apart too, and one a line leads to in it', () => {
    const parts = partsOf(graph, view, new Set(['search']));
    expect(parts.get('a')).toBe(MAIN);
    expect(parts.get('x')).toBe(MAIN);
    expect(parts.get('q')).toBe(ASKING);
    expect(parts.get('t')).toBe('t');
    expect(c.parts).toEqual([MAIN, 'd', 't', 'p', ASKING]);
    expect(c.groups.map((g) => g.id)).toEqual(['stage:start', 'stage:build', ASKING, 'stage:done']);
  });

  it("gives a stage's box the letter its cells' codes start with, and none where they are not codes", () => {
    expect([letterOf('T6'), letterOf('P10'), letterOf('S0'), letterOf('a'), letterOf('t1')]).toEqual(['T', 'P', 'S', '', '']);
    const ids: Array<[string, string]> = [
      ['"a"', '"B1"'],
      ['"b"', '"B2"'],
      ['"c"', '"B3"'],
      ['"d"', '"B4"'],
      ['"m"', '"B5"'],
    ];
    const code = <T,>(x: T): T => JSON.parse(ids.reduce((j, [from, to]) => j.replaceAll(from, to), JSON.stringify(x))) as T;
    const coded = chart(code(graph), code(view));
    expect(coded.groups.find((g) => g.id === 'stage:build')?.letter).toBe('B');
    expect(c.groups.every((g) => g.letter === '')).toBe(true);
  });

  it('draws the actions on rows into the same cell as one node on their way into it, in its box and laid out before it', () => {
    expect(node('a:b:lock+post')).toMatchObject({ kind: 'action', cell: 'b', actions: ['lock', 'post'], group: 'stage:build' });
    const at = (id: string) => c.nodes.findIndex((n) => n.id === id);
    expect(at('a:b:lock+post')).toBe(at('c:b') - 1);
    expect(at('a:m:notify')).toBe(at('c:m') - 1);
    expect(edge('c:a->a:b:lock+post')).toMatchObject({ label: 'done', rows: [1], main: true });
    expect(edge('c:c->a:b:lock+post')).toMatchObject({ label: 'ok', rows: [1] });
    expect(edge('a:b:lock+post->c:b')).toMatchObject({
      label: '',
      through: [
        { cell: 'a', row: 1 },
        { cell: 'c', row: 1 },
      ],
      main: true,
    });
    // A row with no action goes straight; one that waits where it is goes through its action and back.
    expect(edge('c:a->c:b')).toMatchObject({ label: 'again' });
    expect(edge('a:m:notify->c:m')?.through).toEqual([{ cell: 'm', row: 0 }]);
  });

  it('puts what a cell does every time beside it, and what every agent cell does in the legend', () => {
    expect(node('d:b')).toMatchObject({ kind: 'does', actions: ['update_card'] });
    expect(edge('c:b->d:b')?.kind).toBe('does');
    expect(node('d:t')?.actions).toEqual(['record']);
    expect(c.shared).toEqual([{ action: 'tell_owner', at: 'tell_owner' }]);
    expect(node('d:a')).toBeUndefined();
  });

  it('says under a step the engine decides what its values read', () => {
    expect(c.notes.get('m')?.reads).toBe('dependencies merged');
    expect(chart(graph, view).notes.get('m')).toBeUndefined();
  });

  it("names an arrow's rows with no numbers, as a cell's page does, and gives no label to one with no names", () => {
    // The arrow out of the actions on a line has no label at all.
    const onward = edge('a:b:lock+post->c:b');
    expect(onward).not.toHaveProperty('numbers');
    expect(onward && labelSize(onward)).toBeNull();
  });

  it("badges an agent's cell, a gate's too, with the effort it runs at, and no engine, tool or owner cell", () => {
    const at = (effort: string | null, extra: Partial<CompiledCell> = {}) => effortOf(cell('e', [], { effort, ...extra }));
    expect(at('medium')).toEqual({ level: 'medium', text: 'med', cls: 'fc-effort-medium', title: 'Reasoning effort: medium' });
    expect(at('xhigh', { gate: true })).toMatchObject({ text: 'xhigh', cls: 'fc-effort-xhigh' });
    // Every level the flow's compiler allows has a colour of its own; one the page does not know is drawn in grey.
    expect(EFFORTS.map(([level]) => level)).toEqual(['low', 'medium', 'high', 'xhigh', 'max']);
    for (const [level] of EFFORTS) expect(at(level)?.cls).toBe(`fc-effort-${level}`);
    expect(at('turbo')).toMatchObject({ text: 'turbo', cls: 'fc-effort-other' });
    // A cell of a flow version stored before cells declared an effort has none.
    expect(at(null)).toBeNull();
    expect(effortOf(cell('e', []))).toBeNull();
    for (const kind of ['engine', 'tool', 'owner']) expect(at('high', { kind }), kind).toBeNull();
    // The legend gives the levels the chart draws, each once, low to high, one it does not know last.
    const cells = ['max', 'turbo', 'low', 'high', 'low', 'medium', 'xhigh'].map((e, i) => cell(`c${i}`, [], { effort: e }));
    expect(effortsOf(cells).map((e) => e.level)).toEqual(['low', 'medium', 'high', 'xhigh', 'max', 'turbo']);
    expect(effortsOf(graph.cells)).toEqual([]);
  });

  it('badges an agent cell on no line, and no other, as starting a new session every round, after its effort', () => {
    const anew = cell('p', [], { line: null, effort: 'xhigh' });
    expect(startsAnew(anew)).toBe(true);
    expect(styleOf(anew)).toBe('fresh');
    // A cell of a line that starts a new session at its first round in a work continues it after: no badge.
    expect(startsAnew(cell('r', [], { new_session: true }))).toBe(false);
    expect(startsAnew(cell('e', []))).toBe(false);
    for (const kind of ['engine', 'tool', 'owner']) {
      const c = cell('k', [], { kind, line: null });
      expect(startsAnew(c), kind).toBe(false);
      expect(styleOf(c), kind).toBe(`kind-${kind}`);
    }
    // The badges over the top left corner, from 4 px left of the cell: the effort's, then 3 px on the new session's.
    expect(sessionWidth()).toBeGreaterThan(effortWidth());
    expect(cornerWidth(anew)).toBe(effortWidth() + 3 + sessionWidth() - 4);
    expect(cornerWidth(cell('n', [], { line: null }))).toBe(sessionWidth() - 4);
    expect(cornerWidth(cell('e', [], { effort: 'high' }))).toBe(effortWidth() - 4);
    expect(cornerWidth(cell('t', [], { kind: 'tool' }))).toBe(0);
  });

  it("keeps an arrow's label on its own line", () => {
    const route = (label: { x: number; y: number }) => ({
      points: [
        { x: 100, y: 100 },
        { x: 100, y: 300 },
        { x: 160, y: 300 },
      ],
      label: { ...label, width: 40, height: 20 },
      hops: [],
    });
    // Where elkjs put it on the line, it stays.
    expect(onLine(route({ x: 80, y: 190 })).label).toEqual({ x: 80, y: 190, width: 40, height: 20 });
    // Left at a stage box's corner, it goes to the middle of the longest piece.
    expect(onLine(route({ x: 0, y: 0 })).label).toEqual({ x: 80, y: 190, width: 40, height: 20 });
  });

  it('lights up the row pointed at from its source to its target, through actions and circles, and no other', () => {
    expect(related(c.edges, 'c:a->a:b:lock+post')).toEqual(new Set(['c:a->a:b:lock+post', 'a:b:lock+post->c:b']));
    expect(related(c.edges, 'a:b:lock+post->c:b')).toEqual(
      new Set(['a:b:lock+post->c:b', 'c:a->a:b:lock+post', 'c:c->a:b:lock+post']),
    );
    // The asking part answers back into each cell whose own answer asked: pointing at the way into b lights that way
    // alone.
    expect(related(c.edges, 'i:q:b->c:b')).toEqual(new Set(['i:q:b->c:b', 'c:q->o:b:q']));
    expect(related(c.edges, 'c:q->o:c:q')).toEqual(new Set(['c:q->o:c:q', 'i:q:c->c:c']));
    expect(related(c.edges, 'c:t->o:r:t')).toEqual(new Set(['c:t->o:r:t']));
    expect(related(c.edges, 'r:tree:a->c:a')).toEqual(new Set(['r:tree:a->c:a']));
    expect(related(c.edges, null).size).toBe(0);
  });

  it('draws a row to another part as a circle beside the cell, and one beside the cell it comes back into', () => {
    expect(node('o:r:t')).toMatchObject({ kind: 'out', cell: 't', part: 't' });
    expect(node('o:b:q')).toMatchObject({ kind: 'out', cell: 'q', part: ASKING });
    expect(node('i:q:b')).toMatchObject({ kind: 'in', cell: 'b', part: MAIN });
    // The way back from an ask a cell made by a move, which the legend names, is drawn for no cell.
    expect(node('o:a:q')).toBeUndefined();
    expect(node('i:q:a')).toBeUndefined();
    // Back to where the work stopped: from the cell whose row decided it.
    expect(node('i:t:a')).toMatchObject({ kind: 'in', cell: 'a' });
    expect(edge('i:t:a->c:a')).toMatchObject({ label: 'failed' });
  });


  it('puts the rules beside the cells given them and the interjection beside the steps that take one', () => {
    expect(node('r:tree:a')).toMatchObject({ kind: 'rule', group: 'stage:build' });
    // A rule every cell of a line is given is listed in the cell's inputs, not drawn beside it.
    expect(node('r:general:a')).toBeUndefined();
    expect(edge('r:tree:a->c:a')?.kind).toBe('rule');
    expect(node('j:z')?.kind).toBe('interjection');
    expect(node('j:s')).toBeUndefined();
  });

  it('says which node reads or writes each store, and lays the stores out by what they belong to', () => {
    const links = storeLinks(engine, graph, c);
    expect(links.filter((l) => l.node === 'a:b:lock+post')).toEqual([
      { node: 'a:b:lock+post', store: 'acceptance', write: true },
      { node: 'a:b:lock+post', store: 'cards', write: true },
      { node: 'a:b:lock+post', store: 'queue', write: true },
    ]);
    expect(links.filter((l) => l.node === 'c:m')).toEqual([
      { node: 'c:m', store: 'cards', write: false },
      { node: 'c:m', store: 'merged', write: false },
    ]);
    expect(links.filter((l) => l.node === 'd:b')).toEqual([{ node: 'd:b', store: 'cards', write: true }]);
    const uses = storeUses(engine, graph, 'cards');
    expect(uses.writes.map((w) => [w.what, w.at.map((x) => `${x.cell} ${x.row ?? ''}`.trim())])).toEqual([
      ['post', ['a done', 'c ok']],
      ['update_card', ['b']],
    ]);
    expect(uses.reads).toEqual(['m']);
    const laid = layStores(engine.stores ?? {}, () => ({ width: 100, height: 30 }));
    expect(laid.boxes.map((b) => b.id)).toEqual(['project', 'item', 'work']);
    const y = (n: string) => laid.stores.get(n)?.y ?? -1;
    expect(y('queue') < y('acceptance') && y('acceptance') < y('merged') && y('merged') < y('cards')).toBe(true);
    // A write runs from the node to the store, a read from the store to the node, out of their facing sides.
    const at = { x: 0, y: 0, width: 50, height: 20 };
    const store = { x: 200, y: 100, width: 60, height: 20 };
    expect(linkPath(at, store, true)).toBe('M 50 10 C 125 10 125 110 200 110');
    expect(linkPath(at, store, false)).toBe('M 200 110 C 125 110 125 10 50 10');
  });

  it('marks what a work did at each cell, and the arrows it took', () => {
    const rounds = [
      round(1, 's', 'took', 'started'),
      round(2, 'a', 'took', 'tools'),
      round(3, 't', 'took', 'ran'),
      round(4, 'a', 'sent_back', null),
      round(5, 'a', 'took', 'done'),
      round(6, 'b', 'failed', null),
      round(7, 'b', 'running', null),
    ];
    const step = { tag: 'waiting for Step', why: 'stopped (you paused it)' };
    const o = overlays(rounds, 'b', { running: true, waits: step });
    expect(o.get('a')).toEqual({ rounds: 3, sentBack: 1, failed: 0, running: false, current: false, waits: null });
    // While a round runs at the cell, the work does not wait there.
    expect(o.get('b')).toEqual({ rounds: 2, sentBack: 0, failed: 1, running: true, current: true, waits: null });
    // A round left open while the engine runs nothing of the work does not run; a work waiting says why at its cell.
    const held = overlays(rounds, 'b', { running: false, waits: step });
    expect(held.get('b')).toEqual({ rounds: 2, sentBack: 0, failed: 1, running: false, current: true, waits: step });
    const w = taken(c, graph, rounds);
    expect(w.get('c:s->c:a')).toBe(1);
    expect(w.get('c:a->a:b:lock+post')).toBe(1);
    expect(w.get('a:b:lock+post->c:b')).toBe(1);
    expect(w.get('c:b->c:z')).toBeUndefined();
    // A round chosen lights the arrows it came in by and those it left by; one that has taken no row yet, only the way
    // in; tools in the legend draw no arrow.
    const none = new Set<string>();
    const done = new Set(['c:a->a:b:lock+post', 'a:b:lock+post->c:b']);
    expect(roundArrows(c, graph, rounds, 2)).toEqual({ in: new Set(['c:s->c:a']), out: none });
    expect(roundArrows(c, graph, rounds, 5)).toEqual({ in: none, out: done });
    expect(roundArrows(c, graph, rounds, 6)).toEqual({ in: done, out: none });
    expect(roundArrows(c, graph, rounds, 7)).toEqual({ in: none, out: none });
  });

  it('lights the rows into a cell and out of it, each whole through actions and circles, and no line that is no row', () => {
    const b = ways(c.edges, 'c:b');
    // Into b: straight from a, through the actions a and c lead into it by, and back from the asking part.
    expect(b.in).toEqual(
      new Set(['c:a->c:b', 'a:b:lock+post->c:b', 'c:a->a:b:lock+post', 'c:c->a:b:lock+post', 'i:q:b->c:b', 'c:q->o:b:q']),
    );
    // Out of b: to the end, back to a and to the asking part; the line to what it does every time is no row.
    expect(b.out).toEqual(new Set(['c:b->c:z', 'c:b->c:a', 'c:b->o:q:b']));
    // Out of a through the actions, on to b; into a, a rule's arrow is no row.
    const a = ways(c.edges, 'c:a');
    expect([...a.out].filter((id) => id.includes('lock'))).toEqual(['c:a->a:b:lock+post', 'a:b:lock+post->c:b']);
    expect(a.in.has('c:s->c:a') && a.in.has('c:z->c:a') && a.in.has('i:t:a->c:a')).toBe(true);
    expect(a.in.has('r:tree:a->c:a')).toBe(false);
    expect(ways(c.edges, null)).toEqual({ in: new Set(), out: new Set() });
  });

  it('lights the cell picked, or the cell under the pointer while it is on one, and only a cell', () => {
    // Nothing pointed at, or the cell picked pointed at: the cell picked.
    expect(litCell(c, 'c:b', null)).toBe('c:b');
    expect(litCell(c, 'c:b', 'c:b')).toBe('c:b');
    // Another cell pointed at: that one alone, until the pointer leaves it.
    expect(litCell(c, 'c:b', 'c:a')).toBe('c:a');
    expect(litCell(c, null, 'c:a')).toBe('c:a');
    // The pointer on what is no cell keeps the cell picked; what is picked lights nothing when it is no cell.
    expect(litCell(c, 'c:b', 'a:b:lock+post')).toBe('c:b');
    expect(litCell(c, 'r:tree:a', null)).toBeNull();
    expect(litCell(c, null, null)).toBeNull();
  });

  it('draws a row by the first that holds: pointed at, the round chosen, into or out of the cell lit, then the rest', () => {
    const e = (id: string) => {
      const x = edge(id);
      if (x === undefined) throw new Error(`no arrow ${id}`);
      return x;
    };
    const none = { in: new Set<string>(), out: new Set<string>() };
    const at = (over: Partial<Parameters<typeof lineKind>[1]>) => ({
      hot: new Set<string>(),
      lit: undefined,
      lighted: none,
      taken: null,
      ...over,
    });
    // The main path as thin as any arrow, in a colour of its own; the rest plain.
    expect(lineKind(e('c:s->c:a'), at({}))).toBe('main');
    expect(lineKind(e('c:a->c:b'), at({}))).toBe('plain');
    // Into and out of the cell lit, over the main path and the rows a work took.
    const b = ways(c.edges, 'c:b');
    expect(lineKind(e('c:a->a:b:lock+post'), at({ lighted: b }))).toBe('in');
    expect(lineKind(e('c:b->c:z'), at({ lighted: b }))).toBe('out');
    expect(lineKind(e('c:b->c:z'), at({ lighted: b, taken: new Map([['c:b->c:z', 2]]) }))).toBe('out');
    expect(lineKind(e('c:s->c:a'), at({ lighted: b }))).toBe('main');
    // A rule's arrow is never lit as a row.
    expect(lineKind(e('r:tree:a->c:a'), at({ lighted: ways(c.edges, 'c:a') }))).toBe('rule');
    // The round chosen over the cell lit; the row pointed at over all.
    const lit = { in: new Set(['c:a->c:b']), out: new Set(['c:b->c:z']) };
    expect(lineKind(e('c:a->c:b'), at({ lighted: b, lit }))).toBe('lit-in');
    expect(lineKind(e('c:b->c:z'), at({ lighted: b, lit }))).toBe('lit-out');
    expect(lineKind(e('c:b->c:z'), at({ lighted: b, lit, hot: new Set(['c:b->c:z']) }))).toBe('hot');
    // On a work's chart: the rows it took, and the rest quiet.
    expect(lineKind(e('c:a->c:b'), at({ taken: new Map([['c:a->c:b', 1]]) }))).toBe('taken');
    expect(lineKind(e('c:s->c:a'), at({ taken: new Map() }))).toBe('quiet');
  });

  it("draws the picked cell's rows in and out, and every cell's title semibold", () => {
    const laid: Laid = {
      nodes: new Map(c.nodes.map((n, i) => [n.id, { x: 0, y: i * 60, width: 120, height: 40 }])),
      groups: new Map(),
      edges: new Map(
        c.edges.map((x, i) => [x.id, { points: [{ x: 200, y: i * 10 }, { x: 200, y: i * 10 + 50 }], label: null, hops: [] }]),
      ),
      parts: new Map(),
      bounds: { x: 0, y: 0, width: 300, height: c.nodes.length * 60 },
    };
    const measured = sizes(graph, c);
    const draw = (selected: string | null) => {
      const marks: Marks = { selected, overlays: null, taken: null, added: new Set(), changed: new Set() };
      const props = { g: graph, chart: c, laid, lines: measured.lines, notes: measured.notes, engine, marks };
      return renderToStaticMarkup(createElement(FlowSvg, props));
    };
    // The classes of an arrow's line, as drawn.
    const looks = (html: string, id: string) => {
      const at = html.indexOf(`<g data-edge="${id.replaceAll('>', '&gt;')}">`);
      expect(at, id).toBeGreaterThanOrEqual(0);
      return /<path class="(fc-edge[^"]*)"/.exec(html.slice(at))?.[1].split(' ').filter((k) => k !== '') ?? [];
    };
    const picked = draw('c:b');
    expect(looks(picked, 'c:a->a:b:lock+post')).toContain('fc-edge-in');
    expect(looks(picked, 'c:q->o:b:q')).toContain('fc-edge-in');
    expect(looks(picked, 'c:b->c:a')).toContain('fc-edge-out');
    expect(looks(picked, 'c:s->c:a')).toContain('fc-edge-main');
    // Let go of, nothing is lit.
    const free = draw(null);
    expect(free.includes('fc-edge-in') || free.includes('fc-edge-out')).toBe(false);
    expect(looks(free, 'c:b->c:a')).toContain('fc-edge-plain');
    // A rule picked lights no row.
    expect(draw('r:tree:a').includes('fc-edge-in')).toBe(false);
    // Every cell's title, its code with it, is drawn and measured semibold.
    expect((free.match(/class="fc-title"/g) ?? []).length).toBe(graph.cells.length);
    expect(chartFonts().title.startsWith('600 ')).toBe(true);
    expect(chartFonts().code.startsWith('600 ')).toBe(true);
  });

  it('lets go of what is picked on a click on the background, not on an arrow, and of nothing a comparison opened', () => {
    const under = (attrs: Record<string, string>): Under => ({
      closest: (selector) => {
        const name = /^\[(.+)\]$/.exec(selector)?.[1] ?? '';
        return name in attrs ? { getAttribute: (n) => attrs[n] ?? null } : null;
      },
    });
    const pick = (attrs: Record<string, string>) => pickAt(under(attrs), ['engineer'], [c]);
    expect(pick({})).toEqual({ kind: 'nothing' });
    expect(pick({ 'data-sheet': '0' })).toEqual({ kind: 'nothing' });
    expect(pick({ 'data-edge': 'c:b->c:z', 'data-sheet': '0' })).toBeNull();
    expect(pick({ 'data-node': 'c:b', 'data-sheet': '0' })).toEqual({ kind: 'cell', graph: 'engineer', cell: 'b' });
    expect(pick({ 'data-store': 'cards' })).toEqual({ kind: 'store', store: 'cards' });
    // What a click on nothing lets go of: what the chart picked, a round whose cell it marks among them, as the panel's
    // close does; not what changed in a comparison, which nothing on the chart picked.
    expect(pickedOnChart({ kind: 'cell', cell: 'b' })).toBe(true);
    expect(pickedOnChart({ kind: 'round', round: 3 })).toBe(true);
    expect(pickedOnChart({ kind: 'store', store: 'cards' })).toBe(true);
    expect(pickedOnChart({ kind: 'changes' })).toBe(false);
    expect(pickedOnChart(null)).toBe(false);
  });
});

describe("the halo on a work's chart", () => {
  const flash = (n: number, cell: string, seen: number) => ({ n, cell, seen });
  it('stays on a round too short to be seen running, then on the cell the work waits at', () => {
    // A tool's round, over in milliseconds, the work then waiting at a for Step.
    const at = (now: number) => haloAt([flash(3, 't', 1000)], now, { at: 'a', runs: false, newest: 3 });
    expect(at(1000)).toEqual({ cell: 't', next: 1000 + HALO_STEP });
    expect(at(1000 + HALO_STEP)).toEqual({ cell: 'a', next: 1000 + HALO_HOLD });
    expect(at(1000 + HALO_HOLD)).toEqual({ cell: null, next: null });
  });

  it('goes through rounds that came in together in order, and stays on the one that runs', () => {
    const both = [flash(3, 't', 1000), flash(4, 'a', 1000)];
    const at = (now: number) => haloAt(both, now, { at: 'a', runs: true, newest: 4 });
    expect(at(1000)).toEqual({ cell: 't', next: 1000 + HALO_STEP });
    expect(at(1000 + HALO_STEP)).toEqual({ cell: 'a', next: null });
    expect(at(60_000).cell).toBe('a');
    // A newer round still running takes it from an older one's hold once that has had its least.
    const later = [flash(3, 't', 1000), flash(4, 'a', 1200)];
    expect(haloAt(later, 1300, { at: 'a', runs: true, newest: 4 }).cell).toBe('t');
    expect(haloAt(later, 1000 + HALO_STEP, { at: 'a', runs: true, newest: 4 }).cell).toBe('a');
  });

  it('is on the cell a round runs at, and nowhere while none does, when no round came in', () => {
    expect(haloAt([], 0, { at: 'a', runs: true, newest: 7 })).toEqual({ cell: 'a', next: null });
    expect(haloAt([], 0, { at: 'a', runs: false, newest: 7 })).toEqual({ cell: null, next: null });
  });
});

describe('the map of a chart', () => {
  // Numbers to a millionth, so that a float's last bit does not count.
  const near = <T extends Record<string, number>>(o: T | null): T | null =>
    o === null ? null : (Object.fromEntries(Object.entries(o).map(([k, n]) => [k, Math.round(n * 1e6) / 1e6])) as T);
  const wide = mapGeom({ width: 4000, height: 1000 });
  const seen = { width: 800, height: 600 };
  // At half size, the chart's top left corner 100 px left of the viewer's and 50 px above it.
  const v = { s: 0.5, x: -100, y: -50 };

  it("is as wide as it may be, or as tall, in the chart's proportions", () => {
    expect(near({ k: wide.k, width: wide.width, height: wide.height })).toEqual({ k: 0.05, width: 200, height: 50 });
    const tall = mapGeom({ width: 1000, height: 4000 });
    expect(near({ k: tall.k, width: tall.width, height: tall.height })).toEqual({ k: 0.04, width: 40, height: 160 });
  });

  it('frames the part of the chart in view, cut to the chart, and none when none of it is', () => {
    // The chart from 200 to 1800 px across and from 100 px down to its foot, at a twentieth.
    expect(near(viewFrame(wide, v, seen))).toEqual({ x: 10, y: 5, width: 80, height: 45 });
    // A view past the chart's right edge is cut there.
    expect(near(viewFrame(wide, { s: 1, x: -3600, y: 0 }, seen))).toEqual({ x: 180, y: 0, width: 20, height: 30 });
    expect(viewFrame(wide, { s: 1, x: -5000, y: 0 }, seen)).toBeNull();
  });

  it('brings the point clicked to the middle at the zoom the chart has, and moves the view as the frame is dragged', () => {
    // The map's point 100, 25 is the chart's 2000, 500, which the view puts in the middle, at half size still.
    const to = centredAt(wide, 100, 25, v, seen);
    expect(near(to)).toEqual({ s: 0.5, x: -600, y: 50 });
    const f = viewFrame(wide, to, seen);
    expect(f && Math.round((f.x + f.width / 2) * 1e6) / 1e6).toBe(100);
    // Dragging the frame 10 px right and 4 px up moves the chart 200 px left and 80 px down, at half size.
    expect(near(draggedBy(wide, 10, -4, v))).toEqual({ s: 0.5, x: -200, y: -10 });
  });

  it('shows while some of the chart is out of view, clear of the middle where the chart centres a running cell', () => {
    // The whole chart in view: no map.
    expect(mapShown(wide, { s: 0.15, x: 100, y: 200 }, seen, 212, 172)).toBe(false);
    expect(mapShown(wide, v, seen, 212, 172)).toBe(true);
    // Too small for the map to keep to the left of the middle or above it; wide or tall enough for either.
    expect(mapShown(wide, v, { width: 600, height: 400 }, 212, 172)).toBe(false);
    expect(mapShown(wide, v, { width: 760, height: 400 }, 212, 172)).toBe(true);
    expect(mapShown(wide, v, { width: 600, height: 480 }, 212, 172)).toBe(true);
    // Before the viewer has a size.
    expect(mapShown(wide, v, { width: 0, height: 0 }, 212, 172)).toBe(false);
  });

  it("draws the stage boxes, the cells and the stores where the chart has them, and a work's marks", () => {
    const c = chart(graph, view, engine);
    const box = (x: number, y: number, width: number, height: number): Box => ({ x, y, width, height });
    const laid: Laid = {
      nodes: new Map([
        ['c:a', box(100, 100, 200, 60)],
        ['c:b', box(100, 300, 200, 10)],
        ['a:b:lock+post', box(100, 220, 120, 30)],
      ]),
      groups: new Map([['stage:build', box(80, 60, 400, 400)]]),
      edges: new Map(),
      parts: new Map(),
      bounds: box(0, 0, 1000, 800),
    };
    const stores = {
      boxes: [{ id: 'work', box: box(0, 0, 100, 200) }],
      stores: new Map([['cards', box(10, 34, 80, 30)]]),
      width: 100,
      height: 200,
    };
    const sheet = { graph: 'g', chart: c, laid, left: 1000, overlays: null };
    // A chart of 2000 by 1000 px at a tenth; the graph's sheet at 1000 px, its drawing 20 px in, the sheets 40 px down.
    const map = mapOf({ width: 2000, height: 1000 }, [sheet], 40, 20, { laid: stores, left: 0 });
    const shapes = map.shapes.map((x) => ({ ...x, ...near(box(x.x, x.y, x.width, x.height)) }));
    expect(shapes).toEqual([
      { kind: 'box', graph: 'g', cell: null, mark: null, x: 110, y: 12, width: 40, height: 40 },
      { kind: 'box', graph: null, cell: null, mark: null, x: 2, y: 6, width: 10, height: 20 },
      { kind: 'cell', graph: 'g', cell: 'a', mark: null, x: 112, y: 16, width: 20, height: 6 },
      // A cell thinner than the least a mark is drawn at keeps its middle.
      { kind: 'cell', graph: 'g', cell: 'b', mark: null, x: 112, y: 35.5, width: 20, height: LEAST },
      { kind: 'cell', graph: null, cell: null, mark: null, x: 3, y: 9.4, width: 8, height: 3 },
    ]);
    // On a work's chart: the cell the work is at, over the rest, and the cells it never reached.
    const at = { rounds: 1, sentBack: 0, failed: 0, running: true, current: true, waits: null };
    const worked = (o: typeof at) => mapOf({ width: 2000, height: 1000 }, [{ ...sheet, overlays: new Map([['b', o]]) }], 40, 20, null);
    expect(worked(at).shapes.map((x) => [x.kind, x.cell, x.mark])).toEqual([
      ['box', null, null],
      ['cell', 'a', 'unvisited'],
      ['cell', 'b', 'running'],
    ]);
    expect(worked({ ...at, running: false }).shapes.find((x) => x.cell === 'b')?.mark).toBe('current');
  });
});

describe('the routes of arrows', () => {
  it('round their bends and arch where they cross another arrow', () => {
    const across: Routed = { points: [{ x: 0, y: 50 }, { x: 100, y: 50 }], label: null, hops: [] };
    const down: Routed = { points: [{ x: 50, y: 0 }, { x: 50, y: 100 }], label: null, hops: [] };
    const bent: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 40, y: 40 }], label: null, hops: [] };
    const edges = new Map([
      ['across', across],
      ['down', down],
    ]);
    addHops(edges);
    expect(across.hops).toEqual([[50]]);
    expect(down.hops).toEqual([[]]);
    expect(pathOf(across)).toBe('M 0 50 L 45 50 A 5 5 0 0 1 55 50 L 100 50');
    expect(pathOf(bent)).toBe('M 0 0 L 0 31 Q 0 40 9 40 L 40 40');
  });

  it('end where the head starts, the last piece straight before it, and at the ring as they start there', () => {
    const bent: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 40, y: 40 }], label: null, hops: [] };
    expect(pathOf(bent, { end: 7 })).toBe('M 0 0 L 0 31 Q 0 40 9 40 L 33 40');
    const close: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 12, y: 40 }], label: null, hops: [] };
    expect(pathOf(close, { end: 7 })).toBe('M 0 0 L 0 38 Q 0 40 2 40 L 5 40');
    expect(pathOf(bent, { start: 7, end: 7 })).toBe('M 0 7 L 0 31 Q 0 40 9 40 L 33 40');
  });

  it('give a loop back to its own cell a last piece long enough for a head', () => {
    const loop = (label: Routed['label']): Routed => ({
      points: [{ x: 10, y: 0 }, { x: 0, y: 0 }, { x: 0, y: 20 }, { x: 10, y: 20 }],
      label,
      hops: [],
    });
    const far = { x: -40, y: 5, width: 20, height: 10 };
    const near = { x: -20, y: 5, width: 18, height: 10 };
    expect(lengthenLast(loop(far), 16)).toEqual({
      points: [{ x: 10, y: 0 }, { x: -6, y: 0 }, { x: -6, y: 20 }, { x: 10, y: 20 }],
      label: far,
      hops: [],
    });
    expect(lengthenLast(loop(near), 16).label).toEqual({ ...near, x: -26 });
    // A route of three points cannot move the piece that starts at its source.
    const three: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 5, y: 40 }], label: null, hops: [] };
    expect(lengthenLast(three, 16)).toBe(three);
  });
});

describe("a cell's panel", () => {
  const cellOf = (id: string) => graph.cells.find((x) => x.id === id) ?? graph.cells[0];
  const into = (id: string) => view.cells.find((x) => x.id === id)?.entered_from ?? [];
  it('is one page, with its rounds on a work\'s chart and the changes when two flows are compared as tabs beside it', () => {
    expect(cellTabs({ rounds: false, diff: false })).toEqual(['cell']);
    expect(cellTabs({ rounds: true, diff: true })).toEqual(['changes', 'rounds', 'cell']);
    expect(cellTab(['cell'], 'rounds')).toBe('cell');
    expect(cellTab(['rounds', 'cell'], 'cell')).toBe('cell');
    expect(cellTab(['rounds', 'cell'], null)).toBe('rounds');
  });

  it('says after the title where the cell stands, and its line only when the chart has no colour for it', () => {
    expect(cellWhere(graph, cellOf('a'))).toBe('Stage: build');
    expect(cellWhere(graph, cellOf('c'))).toBe('Stage: build; on the observer line');
    expect(cellWhere(graph, cellOf('q'))).toBe('Asking part: ask');
    expect(cellWhere(graph, cellOf('t'))).toBeNull();
  });

  it('lists where a cell is entered from and where it leaves to by kind, the fallback apart', () => {
    const a = enteredFrom(graph, into('a'), 'a');
    expect(a.answers.map((r) => `${r.cell}:${r.row}`)).toEqual(['s:0', 'b:2', 'r:1', 'z:0']);
    expect(a.back.map((r) => `${r.cell}:${r.row}`)).toEqual(['t:0', 'x:0', 'p:0', 'q:0', 'r:0']);
    expect(a.fallback).toBe(true);
    expect(enteredFrom(graph, into('t'), 't').moves).toEqual([{ row: 'tools', cells: ['a', 'b', 'c', 'd'] }]);
    expect(leavesTo(cellOf('a'))).toEqual({ answers: [1, 2, 3], moves: [0, 4, 5, 6], fallback: [7] });
    // The main path first.
    expect(leavesTo(cellOf('m')).answers).toEqual([1, 0]);
    expect(fallbackWords(graph, sentBack)).toBe('Sent back to be written again.');
  });

  it('lists only the moves of its own the legend does not say, and the rest in one line', () => {
    // A search is drawn as a line and listed; asks, tools and pauses are as in the legend.
    expect(ownMoves(graph, cellOf('a'))).toEqual({ rows: [5], legend: ['ask_q', 'tools', 'pause'], lacks: [] });
    // A cell the legend marks as not asking says so.
    const d = ownMoves(graph, cellOf('d'));
    expect([d.rows, d.legend, d.lacks.map((m) => m.row)]).toEqual([[], ['tools', 'pause'], ['ask_q']]);
    // A counter's description once, on the first row that counts it.
    const k = cell('k', [row('x', 'a', { counts: 'asks' }), row('y', 'b', { counts: 'asks' }), row('z', 'c', { counts: 'turns' })]);
    expect(counterFirst(k, [1, 0, 2])).toEqual(new Set([1, 2]));
  });

  it('puts what the engine does every time the return holds a field beside that field, and the rest as what reads it', () => {
    const uses: Use[] = [
      { kind: 'check', cell: 'h', check: 'nonempty' },
      { kind: 'always', cell: 'h', view: null, action: 'apply_effects' },
      { kind: 'always', cell: 'h', view: 'open_cards', action: null },
      { kind: 'always', cell: 'g', view: null, action: 'interject_card' },
    ];
    expect(fieldActions(uses, 'h')).toEqual(['apply_effects']);
  });

  it('shows a condition only where it says more than the row', () => {
    expect(telling(cellOf('a').rows[1])).toEqual([]);
    expect(telling(asks)).toEqual([]);
    expect(telling(cellOf('m').rows[0])).toEqual([['inp', 'gates', ['waiting']]]);
  });

  it('numbers the inputs as the message does, leaving out those the agent reads itself', () => {
    const c = cell('x', [], {
      inputs: [
        { id: null, what: 'the item', from: [{ db: 'item_body', by: 'item' }], maybe: false },
        { id: null, what: 'the code', from: ['self'], maybe: false },
        { id: null, what: 'the design', from: [['D2', 'design']], maybe: false },
      ],
    });
    expect(numbered(c).map((x) => x.no)).toEqual([1, null, 2]);
  });

  it('heads an input by its name, or in a flow stored before inputs had names by the first words of what it is', () => {
    const x = (what: string, name?: string) => ({ id: null, name, what, from: [], maybe: false });
    expect(inputName(x('The item itself: its problem, why', 'The item itself'))).toBe('The item itself');
    expect(inputName(x("The project's AGENTS.md, and the index of the design tree"))).toBe("The project's AGENTS.md");
    expect(inputName(x('What already exists for this item, when it was worked on before'))).toBe('What already exists for this item');
    expect(inputName(x('The results of the tools you asked for'))).toBe('The results of the tools you asked for');
  });

  it('heads a field by the title the flow gives its place, a branch\'s field by that of its place in any branch', () => {
    const titles = { 'understanding.grounds[verified].claim': 'Claim', 'understanding.grounds[verified].where': 'Where checked' };
    expect(titleOf(titles, 'understanding.grounds[verified].where', 'where')).toBe('Where checked');
    expect(titleOf(titles, 'understanding.grounds[inferred].claim', 'claim')).toBe('Claim');
    // A flow version stored before fields had titles: the key in words.
    expect(titleOf(undefined, 'understanding.vs_item', 'vs_item')).toBe('Vs item');
  });

  it('gives once the note every field shown on the item is told, and none when fewer than two fields say it', () => {
    const note = 'Write it in the owner\'s language. Keep technical terms in their English original.';
    const descs: Record<string, string> = {
      'a:understanding': `How you understand this work. ${note}`,
      'b:design': `The design you chose. ${note}`,
    };
    const g = { ...graph, cells: [cell('a', [], { kind: 'agent', shown: ['understanding'] }), cell('b', [], { kind: 'agent', shown: ['design'] })] };
    expect(shownNote(g, (c, p) => descs[`${c}:${p}`] ?? null)).toBe(note);
    const one = { ...graph, cells: [cell('a', [], { kind: 'agent', shown: ['understanding'] })] };
    expect(shownNote(one, (c, p) => descs[`${c}:${p}`] ?? null)).toBeNull();
  });
});

describe("a work's rounds", () => {
  it('are shown all, or only those sent back, or only those that failed or were cut off', () => {
    const rounds = [
      round(1, 'a', 'took', 'done'),
      round(2, 'a', 'sent_back', null),
      round(3, 'a', 'failed', null),
      round(4, 'a', 'interrupted', null),
    ];
    expect(roundsShown(rounds, null)).toHaveLength(4);
    expect(roundsShown(rounds, 'sent_back').map((r) => r.n)).toEqual([2]);
    expect(roundsShown(rounds, 'failed').map((r) => r.n)).toEqual([3, 4]);
  });
});

describe('an ask', () => {
  it('keeps a command in its text as code, and the rest as prose', () => {
    const what = [
      'Please run a short probe on the Mac the engine runs on.',
      '',
      "python3 - <data dir> <<'EOF'",
      'import fcntl, os, sys',
      'fd = os.open(sys.argv[1], os.O_RDONLY)',
      'EOF',
      '',
      'Expected: line 1 taken\u3002',
      '',
      '```',
      'one line of code.',
      '```',
    ].join('\n');
    expect(parseMarkdown(what)).toEqual([
      { kind: 'para', text: 'Please run a short probe on the Mac the engine runs on.' },
      { kind: 'code', text: "python3 - <data dir> <<'EOF'\nimport fcntl, os, sys\nfd = os.open(sys.argv[1], os.O_RDONLY)\nEOF" },
      { kind: 'para', text: 'Expected: line 1 taken\u3002' },
      { kind: 'code', text: 'one line of code.' },
    ]);
  });
});

describe('a round', () => {
  it('reads the numbers of the list to work from, names a model as people do, and reads a value at an address', () => {
    const workFrom = ['1. R6-12·D2·ask', 'Read yourself: the code', '3. R6-15·engine·work_counters', 'Rules: rule:general'];
    expect(addressesByNumber(workFrom)).toEqual(
      new Map([
        [1, ['R6-12·D2·ask']],
        [3, ['R6-15·engine·work_counters']],
      ]),
    );
    expect(modelName('claude-opus-5-5')).toBe('Opus 5.5');
    expect(modelName('gpt-x')).toBe('gpt-x');
    const out = { design: 'd', end: { test_changes: [{ kind: 'added', name: 'a' }, { kind: 'removed', name: 'b' }] } };
    expect(atPath(out, 'design')).toBe('d');
    expect(atPath(out, 'end.test_changes[added]')).toEqual([{ kind: 'added', name: 'a' }]);
    expect(atPath(out, 'end.test_changes.name')).toEqual(['a', 'b']);
    expect(atPath(out, 'missing.path')).toBeUndefined();
  });

  it('cuts a message at its headings and keeps every character', () => {
    const message = '# R9-12 · cell D3\n\nCame from cell D2.\n\n## Given\n\n### format:D3\n\nThe answer\n## Task\nThink.\n';
    const parts = messageParts(message);
    expect(parts.filter((p) => 'heading' in p)).toEqual([
      { heading: '# R9-12 · cell D3' },
      { heading: '## Given' },
      { heading: '### format:D3' },
      { heading: '## Task' },
    ]);
    expect(parts.map((p) => ('heading' in p ? p.heading : p.text)).join('\n')).toBe(message);
    expect(messageParts('')).toEqual([{ text: '' }]);
  });

  it("says what a folded part of a message holds: its size and its share of the message, and for the task that it is the cell's", () => {
    expect(shareWords(14730, 17330, false)).toBe(', 85% of the message');
    expect(shareWords(1200, 900000, false)).toBe(', <1% of the message');
    expect(shareWords(1402, 12277, true)).toBe(", 11% of the message, the cell's task, the same every round");
    expect(chars('一二三')).toBe(3);
  });

  it("heads each field handed back by the title of its place, the forms of a return at the places' root", () => {
    const better: JsonValue = {
      description: 'Is there a better design',
      oneOf: [
        {
          type: 'object',
          properties: { verdict: { const: 'yes', description: 'Yes: change it' }, what: { type: 'string', description: 'What' } },
        },
        {
          type: 'object',
          properties: {
            verdict: { const: 'no', description: 'The present one is best' },
            considered: { type: 'array', items: { type: 'string' }, description: 'Which approaches you considered' },
          },
        },
      ],
    };
    const schema: JsonValue = {
      type: 'object',
      properties: {
        outcome: {
          description: 'What this round comes to: exactly one',
          anyOf: [
            { type: 'object', properties: { better }, required: ['better'], additionalProperties: false },
            { type: 'object', properties: { pause: { type: 'object' } }, required: ['pause'], additionalProperties: false },
          ],
        },
        progress: { type: 'string', description: 'One sentence' },
      },
    };
    const titles = { better: 'A better design', 'better[no].considered': 'Approaches considered', progress: 'Progress note' };
    const shown = shape({ outcome: { better: { verdict: 'no', considered: ['one', 'two'] } }, progress: 'Done.' }, schema, {
      within: 'outcome',
      titles,
    });
    expect(shown).toEqual({
      kind: 'fields',
      fields: [
        {
          key: 'outcome',
          title: null,
          value: {
            kind: 'fields',
            fields: [
              {
                key: 'better',
                title: 'A better design',
                value: {
                  kind: 'fields',
                  fields: [
                    // A value the schema fixes is a kind; its place has no title, so its key in words heads it.
                    { key: 'verdict', title: 'Verdict', value: { kind: 'fixed', text: 'no' } },
                    {
                      key: 'considered',
                      title: 'Approaches considered',
                      value: {
                        kind: 'list',
                        items: [
                          { kind: 'text', text: 'one' },
                          { kind: 'text', text: 'two' },
                        ],
                      },
                    },
                  ],
                },
              },
            ],
          },
        },
        { key: 'progress', title: 'Progress note', value: { kind: 'text', text: 'Done.' } },
      ],
    });
    // A pause holds an object with no fields listed: its own keys, by their JSON type, with no title.
    const paused = shape({ outcome: { pause: { why: 'asked', n: 2 } } }, schema, { within: 'outcome', titles });
    const pause = paused.kind === 'fields' && paused.fields[0].value.kind === 'fields' ? paused.fields[0].value.fields[0] : null;
    expect(pause?.value).toEqual({
      kind: 'fields',
      fields: [
        { key: 'why', title: null, value: { kind: 'text', text: 'asked' } },
        { key: 'n', title: null, value: { kind: 'plain', text: '2' } },
      ],
    });
  });

  it("lays out a list of objects item by item, each branch's fields by the titles of its places, and an empty list as empty", () => {
    const schema: JsonValue = {
      type: 'object',
      properties: {
        grounds: {
          type: 'array',
          items: {
            oneOf: [
              { type: 'object', properties: { kind: { const: 'verified' }, claim: { type: 'string' }, where: { type: 'string' } } },
              { type: 'object', properties: { kind: { const: 'inferred' }, claim: { type: 'string' }, from: { type: 'string' } } },
            ],
          },
        },
        side_reports: { type: 'array', items: { type: 'object', properties: { urgent: { type: 'boolean' } } } },
      },
    };
    const titles = {
      grounds: 'What it rests on',
      'grounds[verified].claim': 'Claim',
      'grounds[verified].where': 'Where checked',
      'grounds[inferred].from': 'Follows from',
    };
    const shown = shape(
      { grounds: [{ kind: 'verified', claim: 'a', where: 'here' }, { kind: 'inferred', claim: 'b', from: 'there' }], side_reports: [] },
      schema,
      { titles },
    );
    const grounds = shown.kind === 'fields' ? shown.fields[0] : null;
    expect(grounds?.title).toBe('What it rests on');
    expect(grounds?.value).toEqual({
      kind: 'list',
      items: [
        {
          kind: 'fields',
          fields: [
            { key: 'kind', title: 'Kind', value: { kind: 'fixed', text: 'verified' } },
            { key: 'claim', title: 'Claim', value: { kind: 'text', text: 'a' } },
            { key: 'where', title: 'Where checked', value: { kind: 'text', text: 'here' } },
          ],
        },
        {
          kind: 'fields',
          fields: [
            { key: 'kind', title: 'Kind', value: { kind: 'fixed', text: 'inferred' } },
            // The same field in another branch takes the title of its place in any branch.
            { key: 'claim', title: 'Claim', value: { kind: 'text', text: 'b' } },
            { key: 'from', title: 'Follows from', value: { kind: 'text', text: 'there' } },
          ],
        },
      ],
    });
    expect(shown.kind === 'fields' && shown.fields[1].value).toEqual({ kind: 'empty', text: '[]' });
    // With no schema, a value by its JSON type alone, no field titled.
    expect(shape({ a: [1, 'x'] }, null)).toEqual({
      kind: 'fields',
      fields: [{ key: 'a', title: null, value: { kind: 'list', items: [{ kind: 'plain', text: '1' }, { kind: 'text', text: 'x' }] } }],
    });
  });

  it('finds the schema of the place an address names, through the forms of a return and a list', () => {
    const schema: JsonValue = {
      type: 'object',
      properties: {
        outcome: {
          anyOf: [
            { type: 'object', properties: { design: { type: 'string', description: 'The design itself' } } },
            { type: 'object', properties: { tools: { type: 'array', items: { type: 'object' }, description: 'Tools' } } },
          ],
        },
        results: { type: 'array', items: { type: 'object', properties: { tool: { type: 'string', description: 'Which' } } } },
      },
    };
    expect(placeSchema(schema, 'design')?.description).toBe('The design itself');
    expect(placeSchema(schema, 'tools')?.description).toBe('Tools');
    expect(placeSchema(schema, 'results.tool')?.description).toBe('Which');
    expect(placeSchema(schema, 'missing')).toBeNull();
  });

  it('says where an agent round spent its cost, by the shares of its prices, with the tokens on hovering', () => {
    const x = {
      steps: 50,
      input: 74,
      read: 12509639,
      written_5m: 0,
      written_1h: 71096,
      output: 30127,
      thinking: 14389,
      context_end: 373995,
      window: 1000000,
      context_before: 302901,
      level: '2',
      effort: 'xhigh',
      others: [],
    };
    const s = spent(x, 'claude-sonnet-5-5');
    expect(s.parts.map((p) => p.text).join(' · ')).toBe('50 steps · context 303K→374K · re-read 68% · new 15% · output 16%');
    expect(s.parts.map((p) => p.whole).join(' · ')).toBe(
      '50 steps · context 303K→374K · re-read 12.51M tokens, 68% · newly cached 71K (1h), 15% · output 30K, of it thinking 14K, 16%',
    );
    expect(s.shares !== null && Math.abs(s.shares.read + s.shares.written + s.shares.output - 1) < 0.001).toBe(true);
    expect(spent({ ...x, context_before: null }, 'some-model').parts.map((p) => p.text).join(' · ')).toBe(
      '50 steps · context 374K · re-read 12.51M · new 71K · output 30K',
    );
    expect(tokens(74)).toBe('74');
    expect(tokens(999_700)).toBe('1.00M');
  });
});

describe("a work's usage", () => {
  it('splits its time, groups its models by the level each round recorded, and finds the subagents and the sessions', () => {
    const tokensOf = (extra: Partial<RoundTokens>): RoundTokens => ({
      steps: 2,
      input: 0,
      read: 1_000_000,
      written_5m: 0,
      written_1h: 10_000,
      output: 1_000,
      thinking: 500,
      context_end: 200_000,
      window: 1_000_000,
      context_before: null,
      level: '2',
      effort: 'xhigh',
      others: [],
      ...extra,
    });
    const at = (r: RoundSummary, start: number, end: number, extra: Partial<RoundSummary> = {}): RoundSummary => ({
      ...r,
      started_at: start,
      ended_at: end,
      ...extra,
    });
    const rounds = [
      at(round(1, 'a', 'took', 'tools'), 0, 60_000, { line: 'engineer', model: 'claude-sonnet-5-5', cost_usd: 0.2, tokens: tokensOf({}) }),
      at(round(2, 't', 'took', 'ran'), 60_000, 120_000),
      at(round(3, 'a', 'sent_back', null), 300_000, 360_000, {
        line: 'engineer',
        model: 'claude-sonnet-5-5',
        cost_usd: 0.3,
        tokens: tokensOf({ context_end: 250_000, others: ['claude-haiku-5-5'] }),
      }),
    ];
    const cells = new Map([
      ['a', { kind: 'agent', title: 'Implement' }],
      ['t', { kind: 'tool', title: 'Run tools' }],
    ]);
    const u = usageOf(rounds, cells, 0);
    expect(u.time).toEqual({ model: 120_000, tool: 60_000, engine: 0, idle: 180_000 });
    expect([u.agentRounds, u.sentBack, u.compactions]).toEqual([2, 1, 0]);
    expect(u.models.map((m) => [m.model, m.level, m.effort, m.rounds, m.turns])).toEqual([['claude-sonnet-5-5', '2', 'xhigh', 2, 4]]);
    expect(u.subagents).toEqual([{ model: 'claude-haiku-5-5', rounds: [3] }]);
    expect(u.sessions.map((s) => [s.line, s.context, s.points.length])).toEqual([['engineer', 250_000, 2]]);
    expect(u.tools).toEqual([{ cell: 't', title: 'Run tools', ms: 60_000, runs: 1 }]);
    expect(u.kinds !== null && Math.round(u.kinds.read)).toBe(200_000);
    // Each stretch no round ran is a block of its own, between the rounds around it, with what happened in it.
    expect(gapsOf(u.blocks, [{ at: 200_000, what: 'answered' }, { at: 400_000, what: 'later' }])).toEqual([
      { start: 120_000, end: 300_000, before: 2, after: 3, waited: ['answered'] },
    ]);
  });

  it('lists each session of a line with why it started, and counts the keep-warm pings in the cost', () => {
    const tokensOf = (context_end: number): RoundTokens => ({
      steps: 1,
      input: 0,
      read: 0,
      written_5m: 0,
      written_1h: 0,
      output: 0,
      thinking: 0,
      context_end,
      window: null,
      context_before: null,
      level: null,
      effort: null,
      others: [],
    });
    const agent = (n: number, session: number, context: number, cost: number, new_session: string | null): RoundSummary => ({
      ...round(n, 'a', 'took', 'done'),
      line: 'engineer',
      model: 'm',
      cost_usd: cost,
      tokens: tokensOf(context),
      session,
      new_session,
    });
    const rounds = [agent(1, 1, 300_000, 1, 'line'), agent(2, 1, 520_000, 2, null), agent(3, 2, 40_000, 0.5, 'limit')];
    const sessions = [
      { id: 1, line: 'engineer', started: 'line', ended: 3 },
      { id: 2, line: 'engineer', started: 'limit', ended: null },
    ];
    const ping = {
      id: 1,
      session: 1,
      line: 'engineer',
      cell: 'A3',
      on: 'owner',
      what: "the owner's answer to ask 3",
      started_at: 2,
      ended_at: 3,
      model: 'm',
      cost_usd: 0.05,
      read: 520_000,
      written: 20,
      error: null,
    };
    const u = usageOf(rounds, new Map([['a', { kind: 'agent', title: 'Implement' }]]), 0, sessions, [ping]);
    expect(u.sessions.map((s) => [s.id, s.nth, s.started, s.rounds, s.context, s.before, s.pings])).toEqual([
      [1, 1, 'line', 2, 520_000, null, 1],
      [2, 2, 'limit', 1, 40_000, 520_000, 0],
    ]);
    expect(u.sessions[0].cost).toBeCloseTo(3.05);
    expect([u.pings.length, u.pings[0].nth]).toEqual([1, 1]);
    expect(u.pingCost).toBeCloseTo(0.05);
    expect(u.cost).toBeCloseTo(3.55);
  });

  it('marks the axis of its wall time at a step the width has room for, so that no two labels run into each other', () => {
    // A work that ran a day and twenty-two hours and a half.
    const start = new Date(2026, 9, 27, 5, 12).getTime();
    const end = start + (46 * 60 + 30) * 60_000;
    for (const width of [240, 480, 700, 1400]) {
      const ticks = axisTicks(start, end, width);
      expect(ticks[0]).toEqual({ at: 0, label: 'Oct 27 05:12', end: 'start' });
      expect(ticks[ticks.length - 1]).toEqual({ at: 1, label: 'Oct 29 03:42', end: 'end' });
      for (let i = 1; i < ticks.length; i++) {
        expect((ticks[i].at - ticks[i - 1].at) * width).toBeGreaterThanOrEqual(64);
      }
    }
    // At 700 pixels the marks are six hours apart, one at midnight saying the day, none crowding an end.
    expect(
      axisTicks(start, end, 700)
        .filter((x) => x.end === null)
        .map((x) => x.label),
    ).toEqual(['12:00', '18:00', 'Oct 28', '06:00', '12:00', '18:00']);
    // Twenty minutes in 600 pixels are marked every five, with no day.
    const morning = new Date(2026, 9, 27, 10, 0).getTime();
    expect(axisTicks(morning, morning + 20 * 60_000, 600).map((x) => x.label)).toEqual([
      '10:00',
      '10:05',
      '10:10',
      '10:15',
      '10:20',
    ]);
  });
});

describe('what the agents spent by line and model', () => {
  it('sums it by line and by model, the costliest of each first', () => {
    const row = (model: string, line: string, cost_usd: number) => ({ model, line, rounds: 1, pings: 0, cost_usd });
    const s = spendTable([
      row('claude-opus-5-5', 'engineer', 4),
      row('claude-fable-5-5', 'advisor', 3),
      row('claude-opus-5-5', 'review', 2),
      row('claude-sonnet-5-5', 'advisor', 0.5),
    ]);
    expect(s.models).toEqual(['claude-opus-5-5', 'claude-fable-5-5', 'claude-sonnet-5-5']);
    expect(s.lines.map((l) => [l.key, l.total])).toEqual([
      ['engineer', 4],
      ['advisor', 3.5],
      ['review', 2],
    ]);
    expect(s.byModel['claude-opus-5-5']).toBe(6);
    expect(s.total).toBe(9.5);
  });
});

describe('the settings on the page', () => {
  const settings = {
    tiers: [
      { name: '1', model: 'claude-haiku-5-5', desc: 'small' },
      { name: '2', model: 'claude-sonnet-5-5', desc: 'known' },
      { name: '3', model: 'claude-opus-5-5', desc: 'new' },
      { name: '4', model: 'claude-fable-5-1', desc: 'advisor' },
    ],
    highest_level: '3',
    architect: '3',
    context_limit: 500_000,
    keep_warm_minutes: 55,
    keep_warm_most: 39,
    commit: 'abcdef0123',
  };

  it('sends only what changed and refuses what it can tell is wrong', () => {
    const models = { '1': 'claude-haiku-5-5', '2': 'claude-sonnet-5-5', '3': 'claude-opus-5-5', '4': 'claude-opus-5-5' };
    expect(settingsChange(settings, models, '3', '3', '500000')).toEqual({
      models: { '4': 'claude-opus-5-5' },
      highest_level: null,
      architect: null,
      context_limit: null,
    });
    expect(settingsChange(settings, models, '2', '2', '400000').context_limit).toBe(400_000);
    expect(settingsProblems({ ...models, '2': ' ' }, '0')).toEqual(['Tier 2 needs a model.', 'The size limit is a number of tokens above 0.']);
    expect(settingsProblems(models, '500000')).toEqual([]);
  });

  it('offers an item no tier above the highest', () => {
    expect(tiersAllowed(settings)).toEqual(['1', '2', '3']);
    expect(tiersAllowed({ ...settings, highest_level: null })).toEqual(['1', '2', '3', '4']);
  });
});

describe('a ruling on a design card', () => {
  const unset: Verdict = { verdict: null, text: '', option: '' };

  it('is set decision by decision: the approach recommended confirms, another is chosen, and a second click clears', () => {
    const chose = pickApproach(unset, 'B', 'A');
    expect(chose).toEqual({ verdict: 'disagree', text: '', option: 'B' });
    expect(verdictReady(chose)).toBe(true);
    expect(pickApproach(chose, 'B', 'A')).toEqual(unset);
    expect(pickApproach({ ...chose, text: 'Too slow.' }, 'B', 'A')).toEqual({ verdict: 'disagree', text: 'Too slow.', option: '' });
    expect(pickApproach(chose, 'A', 'A')).toEqual({ verdict: 'confirm', text: '', option: '' });
    expect(verdictReady({ verdict: 'disagree', text: ' ', option: '' })).toBe(false);
    expect(verdictReady(unset)).toBe(false);
  });

  it('is summed up before sending, one line for each decision', () => {
    expect(verdictLine(1, 'confirm', '', '')).toBe('Decision 1: confirm');
    expect(verdictLine(2, 'disagree', 'B', 'One table is enough.')).toBe('Decision 2: disagree, choosing B: One table is enough.');
    expect(verdictLine(3, 'disagree', '', 'No.')).toBe('Decision 3: disagree: No.');
    expect(verdictLine(4, null, '', '')).toBe('Decision 4: not ruled yet');
  });

  it('opens the current card, the one waiting for a ruling or else the newest not replaced, and folds the others', () => {
    const card = (id: number, status: string): DesignCard => ({
      id,
      round: id,
      work: 9,
      item: 4,
      status,
      body: [],
      rulings: [],
      replaced_by: null,
      replaced: null,
      cell: null,
      title: null,
      at: null,
      replaces: null,
      changed: null,
    });
    expect(currentCard([card(6, 'replaced'), card(7, 'ruled')])).toBe(7);
    expect(currentCard([card(6, 'ruled'), card(7, 'open')])).toBe(7);
    expect(currentCard([card(6, 'replaced')])).toBeNull();
  });

  it('titles each decision with its first sentence', () => {
    expect(firstSentence('Keep one table. It is simpler.')).toBe('Keep one table.');
    expect(firstSentence('v1.2 stays\nmore')).toBe('v1.2 stays');
    expect(firstSentence('一二。三')).toBe('一二。');
    expect(firstSentence('x'.repeat(200)).length).toBe(120);
  });
});

describe("the owner's conversations", () => {
  const talk = (key: string, extra: Partial<Conversation> = {}): Conversation => ({
    key,
    kind: key.startsWith('work:') ? 'work' : key.startsWith('architect:') ? 'architect' : key.startsWith('projects:') ? 'projects' : 'wf_agents',
    project: 'p',
    projects: [],
    work: null,
    item: null,
    title: null,
    level: null,
    over: false,
    waiting: 0,
    unread: 0,
    last_at: 1,
    ...extra,
  });
  const entry = (source: string, extra: Partial<ConversationEntry> = {}): ConversationEntry => ({
    source,
    id: 1,
    sender: 'engineer',
    recipient: 'owner',
    text: 'said',
    at: 1,
    work: 5,
    round: 2,
    ask: null,
    design_card: null,
    matter: null,
    read_at: null,
    waiting: false,
    pending: false,
    undelivered: false,
    answerable: false,
    fault: null,
    misses: null,
    forwards: [],
    ask_detail: null,
    revisions: null,
    stage: null,
    matter_card: null,
    policy: null,
    observation: null,
    ...extra,
  });

  it('lists in an order that waiting never changes: the one-to-ones and wf-agents, then the works, then the projects', () => {
    const parts = listParts([
      talk('architect:p', { waiting: 1 }),
      talk('wf-agents', { project: null }),
      talk('work:9', { work: 9, waiting: 2 }),
      talk('work:5', { work: 5, over: true }),
      talk('projects:p|q', { project: null, projects: ['p', 'q'] }),
    ]);
    expect(parts.map((p) => [p.part, p.conversations.map((c) => c.key)])).toEqual([
      ['top', ['architect:p', 'wf-agents']],
      ['works', ['work:9', 'work:5']],
      ['projects', ['projects:p|q']],
    ]);
    expect(totals([talk('work:9', { waiting: 2, unread: 1 }), talk('architect:p', { unread: 3 })])).toEqual({
      waiting: 2,
      unread: 4,
    });
  });

  it('names each conversation by its subject, and each side of an entry and whom a message is for', () => {
    expect(conversationTitle(talk('work:5', { work: 5, item: 3, title: 'Parse it' }))).toBe('#5 Parse it');
    expect(conversationTitle(talk('architect:p'))).toBe('Architect of p');
    expect(conversationTitle(talk('projects:p|q', { projects: ['p', 'q'] }))).toBe('Architects of p, q');
    expect(conversationTitle(talk('wf-agents'))).toBe('wf-agents');
    expect(party(entry('message'), 'owner')).toBe('you');
    expect(party(entry('between'), 'q')).toBe('the architect of q');
    expect(mention(entry('question', { recipient: 'architect' }))).toBe('@architect');
    expect(mention(entry('interjection', { sender: 'owner', recipient: 'both' }))).toBe('@engineer @architect');
    expect(mention(entry('message'))).toBe('@you');
    // In the one-to-one with an architect, the owner's interjection of a flow version before, which said the owner's
    // words to the architect's works while they ran, its policy setting among them, is said to the architect.
    const said = (recipient: string) => entry('interjection', { sender: 'owner', recipient });
    expect(mention(said('architect'), 'architect')).toBe('@architect');
    expect(mention(said('policy'), 'architect')).toBe('@architect');
    expect(mention(said('both'), 'architect')).toBe('@architect');
    expect(mentions(said('policy'), 'architect')).toBe(true);
    expect(mentions(entry('words', { sender: 'owner', recipient: 'architect' }), 'architect')).toBe(false);
    expect(mentions(entry('message'), 'work')).toBe(true);
    // What the policy setting of a flow version before said is the architect's.
    expect(party(entry('message', { sender: 'policy' }), 'policy')).toBe('architect');
  });

  it("draws each entry in its voice, an ask where it was asked, the owner's reply in what it answers, and a stage card's design cards in it", () => {
    const d: ConversationDetail = {
      key: 'work:5',
      entries: [
        entry('message', { id: 1 }),
        entry('question', { id: 2, matter: 2, sender: 'engineer', recipient: 'architect' }),
        entry('reply', { id: 3, matter: 3, sender: 'architect', recipient: 'engineer' }),
        entry('ask', { id: 4, waiting: true }),
        entry('notice', { id: 5, sender: 'engine' }),
        entry('card', { id: 6, design_card: 6, sender: 'engineer' }),
        entry('interjection', { id: 7, sender: 'owner', recipient: 'engineer', at: 3 }),
        entry('message', { id: 8, sender: 'review' }),
        entry('ask', { id: 9 }),
        entry('reply', { id: 2, matter: 2, sender: 'owner', recipient: 'engineer' }),
        entry('message', { id: 10, sender: 'policy' }),
      ],
      asks: [{ id: 4, round: 2, work: 5, item: null, body: {}, kinds: [], answer: null }],
      design_cards: [],
      read_to: null,
      forwarding: null,
    };
    const held = heldCards(d);
    expect(d.entries.map((e) => voiceOf(e, held))).toEqual([
      'agent',
      'agent',
      'agent',
      'agent',
      'system',
      'system',
      'owner',
      'system',
      'agent',
      'owner',
      'agent',
    ]);
    // An ask streams where it was asked; the owner's reply to the engineer's question shows in the question's bubble.
    expect(streamOf(d).map((e) => `${e.source}-${e.id}`)).toEqual([
      'message-1',
      'question-2',
      'reply-3',
      'ask-4',
      'notice-5',
      'card-6',
      'interjection-7',
      'message-8',
      'ask-9',
      'message-10',
    ]);
    expect(latestAt(d)).toBe(3);
    expect(pendingWords([entry('words', { sender: 'owner', pending: true })]).length).toBe(1);
  });

  it('says whom the box at the bottom says things to, and why it cannot at a work at its end', () => {
    expect(sayTo(talk('work:5'))).toEqual(['engineer', 'architect']);
    expect(sayTo(talk('projects:p|q', { projects: ['p', 'q'] }))).toEqual(['p', 'q']);
    // What the ticks come to: both parties of a work's group are both, every architect of a group of projects is all.
    expect(sayToTicked(talk('work:5'), ['engineer'])).toEqual(['engineer']);
    expect(sayToTicked(talk('work:5'), ['engineer', 'architect'])).toEqual(['both']);
    expect(sayToTicked(talk('work:5'), [])).toEqual([]);
    expect(sayToTicked(talk('projects:p|q|r', { projects: ['p', 'q', 'r'] }), ['p', 'q', 'r'])).toEqual(['all']);
    expect(sayToTicked(talk('projects:p|q|r', { projects: ['p', 'q', 'r'] }), ['p', 'r'])).toEqual(['p', 'r']);
    expect(sayTo(talk('wf-agents'))).toEqual([]);
    expect(closedWhy(talk('work:5', { over: true }), null)).not.toBeNull();
    expect(closedWhy(talk('work:5'), null)).toBeNull();
  });

  it("offers a fault to every project's architect, marking those it went to, and says where it went and how it stands", () => {
    const forwarded = (extra: Partial<Forwarded> = {}): Forwarded => ({
      project: 'wf',
      by: 'owner',
      matter: 7,
      state: 'queued',
      handled_work: null,
      handled_round: null,
      ...extra,
    });
    expect(forwardMenu(['p', 'wf'], [forwarded()])).toEqual([
      { project: 'p', label: 'The architect of p', sent: false },
      { project: 'wf', label: 'The architect of wf', sent: true },
    ]);
    expect(forwardMenu(['p'], [])).toEqual([{ project: 'p', label: 'The architect of p', sent: false }]);
    expect(forwardWords(forwarded())).toBe('Forwarded to the architect of wf · in the queue');
    const handled = forwarded({ state: 'handled', handled_work: 12, handled_round: 4 });
    expect(forwardWords(handled)).toBe('Forwarded to the architect of wf · handled in R12-4');
    expect(forwardParts(handled)).toEqual({ lead: 'Forwarded to the architect of wf', state: 'handled in', round: 'R12-4' });
    expect(forwardWords(forwarded({ by: 'setting' }))).toBe('Forwarded automatically to the architect of wf · in the queue');
    // Where each new fault goes: nobody first, as at first, then every project's architect, and the one set even when
    // the configuration no longer has it.
    expect(forwardNewChoices({ projects: ['p', 'wf'], auto: null })).toEqual([
      { value: '', label: 'Nobody' },
      { value: 'p', label: 'The architect of p' },
      { value: 'wf', label: 'The architect of wf' },
    ]);
    expect(forwardNewChoices({ projects: ['p'], auto: 'gone' }).map((c) => c.value)).toEqual(['', 'p', 'gone']);
    expect(t('fault_forward')).toBe('Forward to…');
    expect(t('fault_forward_new')).toBe('Forward new faults to');
  });

  it('tells of what newly waits for the owner, and of nothing the first time', () => {
    const before = waitingByKey([talk('work:5', { waiting: 1 }), talk('architect:p')]);
    expect(newlyWaiting(null, [talk('work:5', { waiting: 1 })])).toEqual([]);
    expect(
      newlyWaiting(before, [talk('work:5', { waiting: 1 }), talk('architect:p', { waiting: 1 }), talk('work:6', { waiting: 1 })]).map(
        (c) => c.key,
      ),
    ).toEqual(['architect:p', 'work:6']);
  });
});
