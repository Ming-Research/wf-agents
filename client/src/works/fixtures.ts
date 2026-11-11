// Works to look at the works page by before the interface serves them, opened with `?fixtures` in the page's address
// (source.ts). Each work is told as what happened to it, step by step, and the rest of what the interface gives (its
// summary, its cells, its blocks with their versions, the contents) is worked out from that, so that the fixtures
// agree with themselves as the interface's would. Times are counted back from when the page loads them.
//
// The works, one for each item: one with a round under way; one waiting for the owner's answer to a question; one
// with a run at its limit of returns sent back; one with a cell at its limit of ways round; one paused; one paused by
// the engine because a round failed; one merged; one cancelled after a let-go; one with a fault; and one long history, waiting at the merge for the owner, with a
// return sent back, tool calls and their results, a round with what it was told and returned, a run abandoned because
// the owner wrote an opinion for an earlier cell, a review's opinions sending the implementation round twice, and a
// write by the outside.

import type { Flow } from '../api';
import { cell, flowOf, test } from '../testing';
import type {
  Ask,
  Block,
  CallPage,
  CallRow,
  Cause,
  CellDetail,
  CellNow,
  CellState,
  Copies,
  Handed,
  Item,
  Items,
  Now,
  Round,
  RoundPage,
  Run,
  Spent,
  Step,
  ToldPage,
  Unmet,
  Version,
  VersionPage,
  WorkPage,
  Writer,
} from '../works-api';

const MINUTE = 60_000;

/// The flow the works run on: a plan, deep research when the plan asks for it, the implementation and its review, a
/// check with the latest main, and the merge, which waits for the owner's approval.
export const FLOW: Flow = flowOf(
  [
    cell('plan', {
      title: 'Plan',
      task: 'Write the plan from the request, and say whether it needs deep research.',
      inputs: ['request', 'opinions_plan'],
      outputs: ['plan', 'needs_deep_research'],
    }),
    cell('deep_research', {
      title: 'Deep research',
      line: 'deep_research',
      task: 'Research what the plan leaves open, and write the solution.',
      condition: { on: 'block', block: 'needs_deep_research', value: 'yes' },
      inputs: ['plan', 'opinions_deep_research'],
      outputs: ['solution'],
    }),
    cell('implement', {
      title: 'Implement',
      task: 'Implement the plan and the solution.\nAnswer what the review asked.',
      inputs: ['plan', 'solution', 'opinions_implement'],
      outputs: ['code', 'answer_review'],
    }),
    cell('review', {
      title: 'Review the code',
      line: 'review',
      task: 'Review the code, and say whether you are content with it.',
      inputs: ['code', 'answer_review'],
      outputs: ['opinions_implement', 'content'],
    }),
    cell('check', {
      title: 'Check with the latest main',
      kind: 'engine',
      line: null,
      task: null,
      act: 'check',
      effort: null,
      inputs: ['code', 'main'],
      outputs: ['ci', 'opinions_implement'],
    }),
    cell('merge', {
      title: 'Merge',
      kind: 'engine',
      line: null,
      task: null,
      act: 'merge',
      effort: null,
      preconditions: [test('approved', 'The owner approved the merge', 'all', ['approval'])],
      inputs: ['ci', 'content', 'approval'],
    }),
  ],
  {
    request: { people: true, shown: true },
    opinions_plan: { people: true },
    plan: {},
    needs_deep_research: { values: ['yes', 'no'] },
    solution: {},
    opinions_deep_research: { people: true },
    code: {},
    answer_review: {},
    opinions_implement: { people: true },
    content: { values: ['yes', 'no'] },
    main: { outside: 'main' },
    ci: { values: ['green', 'red'] },
    approval: { values: ['yes', 'no'], people: true },
  },
  'engineer',
);

/// What a round asks for, with the result its call came to, or none while it has none.
type AskSpec = Ask & { result?: string | null; after?: number };

/// A block a round hands in, with its content.
interface HandSpec {
  block: string;
  content: string;
  value?: string | null;
  /// Whether the run says the change matters; yes unless said.
  matters?: boolean;
}

interface RoundSpec {
  minutes?: number;
  usd?: number;
  asks?: AskSpec[];
  hands?: HandSpec[];
  /// Why what this round hands in is sent back.
  sentBack?: Unmet[];
  /// The round is under way.
  underWay?: boolean;
  /// Why the round failed, where it did: nothing came back.
  fails?: string;
}

const nothing = (): Spent => ({ usdMicros: 0, inputTokens: 0, cachedTokens: 0, outputTokens: 0, workingMs: 0, rounds: 0 });

function add(a: Spent, b: Spent): Spent {
  return {
    usdMicros: a.usdMicros + b.usdMicros,
    inputTokens: a.inputTokens + b.inputTokens,
    cachedTokens: a.cachedTokens + b.cachedTokens,
    outputTokens: a.outputTokens + b.outputTokens,
    workingMs: a.workingMs + b.workingMs,
    rounds: a.rounds + b.rounds,
  };
}

type RunStep = Step & { kind: 'run' };

/// A work told as what happened to it.
class History {
  private place = 0;
  private runs = 0;
  private calls = 0;
  time: number;
  readonly steps: Step[] = [];
  readonly versions = new Map<string, Version[]>();
  readonly contents = new Map<string, string>();
  readonly rounds = new Map<string, RoundPage>();
  readonly callPages = new Map<number, CallPage>();
  /// What a round was told and returned, by its run and number, where it was kept.
  readonly told = new Map<string, ToldPage>();
  private readonly refused = new Map<Round, Unmet[]>();

  constructor(start: number) {
    this.time = start;
  }

  /// The next place, some minutes on.
  private next(minutes = 1): number {
    this.time += Math.round(minutes * MINUTE);
    this.place += 1;
    return this.place;
  }

  current(block: string): Version | undefined {
    const all = this.versions.get(block);
    return all?.[all.length - 1];
  }

  private version(block: string, by: Writer, content: string, value: string | null, matters: boolean): Version {
    const was = this.current(block);
    const choice = FLOW.blocks.find((b) => b.name === block)?.values !== null;
    const changed = !was || (choice ? was.value !== value : matters);
    const v: Version = { at: this.place, time: this.time, by, meaning: changed ? this.place : was.meaning, value, chars: content.length };
    this.versions.set(block, [...(this.versions.get(block) ?? []), v]);
    this.contents.set(`${block}@${v.at}`, content);
    return v;
  }

  wrote(block: string, by: 'person' | 'outside', content: string, value: string | null = null, minutes = 5) {
    const at = this.next(minutes);
    const v = this.version(block, { kind: by }, content, value, true);
    this.steps.push({ kind: 'wrote', at, time: this.time, block, by: { kind: by }, changed: v.meaning === at, value, chars: v.chars });
  }

  did(kind: 'paused' | 'resumed' | 'cancelled', minutes = 5) {
    const at = this.next(minutes);
    const time = this.time;
    if (kind === 'paused') this.steps.push({ kind, at, time, by: 'owner', why: null });
    else if (kind === 'cancelled') this.steps.push({ kind, at, time, why: null });
    else this.steps.push({ kind, at, time });
  }

  /// The engine pauses the work for this reason, as it does in the moment a round fails.
  enginePaused(why: string) {
    const at = this.next(0);
    this.steps.push({ kind: 'paused', at, time: this.time, by: 'engine', why });
  }

  letGo(cellId: string, minutes = 5) {
    const at = this.next(minutes);
    this.steps.push({ kind: 'letGo', at, time: this.time, cell: cellId });
  }

  /// A run of the cell starts, for what its record does not know.
  start(cellId: string, minutes = 1): RunStep {
    const because = this.detail(cellId).needs;
    const at = this.next(minutes);
    const c = FLOW.cells.find((x) => x.id === cellId);
    if (!c) throw new Error(`no cell ${cellId}`);
    const basis = [...new Set([...c.inputs, ...c.outputs])].map((block) => ({
      block,
      version: this.current(block)?.at ?? null,
    }));
    this.runs += 1;
    const run: RunStep = {
      kind: 'run',
      run: this.runs,
      cell: cellId,
      at,
      time: this.time,
      because,
      basis,
      rounds: [],
      calls: [],
      end: null,
      spent: nothing(),
    };
    this.steps.push(run);
    return run;
  }

  round(run: RunStep, spec: RoundSpec = {}): Round {
    const before = run.rounds[run.rounds.length - 1];
    const refused = before ? this.refused.get(before) : undefined;
    const given: Round['given'] = !before
      ? { kind: 'task' }
      : before.back?.kind === 'failed'
        ? before.given
        : refused
        ? { kind: 'sentBack', unmet: refused }
        : before.back?.kind === 'asked'
          ? { kind: 'results', calls: run.calls.slice(-before.back.asks.length).map((c) => c.call) }
          : { kind: 'task' };
    const minutes = spec.minutes ?? 3;
    const round: Round = {
      number: run.rounds.length + 1,
      began: this.time,
      ended: null,
      given,
      back: null,
      spent: null,
      context: null,
      compactions: 0,
    };
    run.rounds.push(round);
    const results = given.kind === 'results' ? given.calls.map((c) => this.callPages.get(c) as CallPage) : [];
    this.rounds.set(`${run.run}/${round.number}`, { run: run.run, number: round.number, results, handed: [] });
    if (spec.underWay) return round;
    this.next(minutes);
    round.ended = this.time;
    const input = Math.round(14_000 + minutes * 3_100);
    const cached = Math.round(input * 0.7);
    const output = Math.round(600 + minutes * 450);
    round.spent = {
      usdMicros: Math.round((spec.usd ?? (input * 3 + cached * 0.3 + output * 15) / 1_000_000) * 1_000_000),
      inputTokens: input,
      cachedTokens: cached,
      outputTokens: output,
      workingMs: minutes * MINUTE,
      rounds: 1,
    };
    round.context = input + cached;
    run.spent = add(run.spent, round.spent);
    if (spec.fails) {
      round.back = { kind: 'failed', why: spec.fails, failure: 'other', resets: null };
      this.enginePaused(spec.fails);
    } else if (spec.asks) {
      round.back = {
        kind: 'asked',
        asks: spec.asks.map((a): Ask =>
          a.kind === 'tool' ? { kind: 'tool', name: a.name, input: a.input } : { kind: 'question', who: a.who, question: a.question },
        ),
      };
      for (const ask of spec.asks) {
        this.calls += 1;
        const what: CallRow['what'] =
          ask.kind === 'tool'
            ? { kind: 'tool', name: ask.name, input: ask.input }
            : { kind: 'question', who: ask.who, question: ask.question };
        const result = ask.result ?? null;
        const row: CallRow = {
          call: this.calls,
          what,
          made: this.time,
          done: result === null ? null : this.time + Math.round((ask.after ?? 0.2) * MINUTE),
          chars: result === null ? null : result.length,
        };
        run.calls.push(row);
        this.callPages.set(row.call, { call: row.call, what, result });
        if (row.done !== null) this.time = Math.max(this.time, row.done);
      }
    } else if (spec.hands) {
      const blocks: Handed[] = spec.hands.map((h) => ({
        block: h.block,
        matters: h.matters ?? true,
        value: h.value ?? null,
        chars: h.content.length,
      }));
      round.back = { kind: 'handedIn', blocks };
      const page = this.rounds.get(`${run.run}/${round.number}`) as RoundPage;
      page.handed = spec.hands.map((h) => ({ block: h.block, content: h.content }));
      if (spec.sentBack) this.refused.set(round, spec.sentBack);
    }
    return round;
  }

  /// The run rests at a call the engine makes for it: the owner letting it go on at its limit.
  atLimit(run: RunStep, limit: CallRow['what'] & { kind: 'atLimit' }) {
    this.calls += 1;
    const row: CallRow = { call: this.calls, what: limit, made: this.time, done: null, chars: null };
    run.calls.push(row);
    this.callPages.set(row.call, { call: row.call, what: limit, result: null });
    return row.call;
  }

  /// The call gets its result.
  answer(run: RunStep, call: number, result: string, minutes = 10) {
    this.next(minutes);
    const row = run.calls.find((c) => c.call === call) as CallRow;
    row.done = this.time;
    row.chars = result.length;
    this.callPages.set(call, { call, what: row.what, result });
  }

  /// The run's last round met everything: it writes what that round handed in.
  finish(run: RunStep, minutes = 0.2) {
    const at = this.next(minutes);
    const last = run.rounds[run.rounds.length - 1];
    const handed = last.back?.kind === 'handedIn' ? last.back.blocks : [];
    const page = this.rounds.get(`${run.run}/${last.number}`) as RoundPage;
    const writes = handed.map((h) => {
      const content = page.handed.find((x) => x.block === h.block)?.content ?? '';
      this.version(h.block, { kind: 'run', run: run.run, cell: run.cell }, content, h.value, h.matters);
      return h;
    });
    run.end = { kind: 'finished', at, time: this.time, writes };
  }

  abandon(run: RunStep, minutes = 0.2) {
    const at = this.next(minutes);
    const last = run.rounds[run.rounds.length - 1];
    if (last && last.ended === null) last.ended = this.time;
    run.end = { kind: 'abandoned', at, time: this.time };
  }

  /// A run of one round that hands these in and finishes.
  ran(cellId: string, hands: HandSpec[], round: RoundSpec = {}): RunStep {
    const run = this.start(cellId);
    this.round(run, { ...round, hands });
    this.finish(run);
    return run;
  }

  /// The meaning a block had at a version.
  private meaningAt(block: string, at: number | null): number | null {
    if (at === null) return null;
    return this.versions.get(block)?.find((v) => v.at === at)?.meaning ?? null;
  }

  private runsOf(cellId: string): RunStep[] {
    return this.steps.filter((s): s is RunStep => s.kind === 'run' && s.cell === cellId);
  }

  /// What the cell's record knows of each block it reads, and why it would run: as the walk works it out.
  detail(cellId: string): CellDetail {
    const c = FLOW.cells.find((x) => x.id === cellId);
    if (!c) throw new Error(`no cell ${cellId}`);
    const order = new Map(FLOW.cells.map((x, i) => [x.id, i]));
    const finished = this.runsOf(cellId).filter((r) => r.end?.kind === 'finished');
    const record = finished[finished.length - 1];
    const inputs = c.inputs.map((block) => {
      const current = this.current(block)?.at ?? null;
      let known: number | null = null;
      if (record?.end?.kind === 'finished') {
        known = record.end.writes.some((w) => w.block === block)
          ? record.end.at
          : (record.basis.find((b) => b.block === block)?.version ?? null);
      }
      const changed = record !== undefined && this.meaningAt(block, current) !== this.meaningAt(block, known);
      return { block, current, known, changed };
    });
    const needs: Cause[] = !record
      ? [{ kind: 'noRecord' }]
      : inputs
          .filter((i) => i.changed)
          .map((i) => {
            const meaning = this.meaningAt(i.block, i.current);
            const by = this.versions.get(i.block)?.find((v) => v.at === meaning)?.by;
            const behind = by?.kind === 'run' && (order.get(by.cell) ?? 0) > (order.get(cellId) ?? 0);
            return { kind: 'changed', block: i.block, was: i.known, behind };
          });
    return { cell: cellId, needs, inputs };
  }

  /// The work as the interface would give it now.
  page(number: number, item: number, title: string, now: Now, paused = false): WorkPage {
    const runs = this.steps.filter((s): s is RunStep => s.kind === 'run');
    const details = FLOW.cells.map((c) => this.detail(c.id));
    const cells: CellNow[] = FLOW.cells.map((c, i) => {
      const mine = this.runsOf(c.id);
      const finished = mine.filter((r) => r.end?.kind === 'finished');
      const record = finished[finished.length - 1];
      const open = mine.find((r) => r.end === null);
      const condition = c.condition;
      const skipped = condition?.on === 'block' && this.current(condition.block)?.value !== condition.value;
      const lastRound = open?.rounds[open.rounds.length - 1];
      let state: CellState = record ? (details[i].needs.length > 0 ? 'again' : 'done') : 'notYet';
      if (open) {
        state = lastRound && lastRound.ended === null ? 'running' : lastRound?.back?.kind === 'failed' ? 'failed' : 'resting';
      }
      if (skipped) state = 'skipped';
      return { cell: c.id, state, finished: finished.length, record: record?.run ?? null, open: open?.run ?? null };
    });
    const blocks: Block[] = [...this.versions.entries()]
      .sort(([a], [b]) => (a < b ? -1 : 1))
      .map(([name, versions]) => ({ name, versions }));
    return {
      summary: {
        number,
        item,
        flow: FLOW.name,
        added: this.steps[0]?.time ?? this.time,
        paused,
        now,
        cells,
        spent: runs.reduce((s, r) => add(s, r.spent), nothing()),
        last: this.steps.length > 0 ? this.time : null,
      },
      title,
      level: '2',
      history: { first: { title, level: '2', dependsOn: [] }, changes: [] },
      flow: FLOW,
      cells: details,
      blocks,
      steps: this.steps,
    };
  }
}


/// The content of a plan, a solution or code, long enough to look like one.
function prose(head: string, lines: string[]): string {
  return [`# ${head}`, '', ...lines.map((l) => `- ${l}`)].join('\n');
}

const REQUEST_TEXT = (what: string) =>
  `${what}\n\nThe owner reads this page all day: keep it dense, and say plainly why the work is where it is.`;

/// The plan and the implementation of a small work, up to a review that is content.
function upToReview(h: History, what: string, deepResearch: boolean) {
  h.wrote('request', 'person', REQUEST_TEXT(what));
  h.ran('plan', [
    { block: 'plan', content: prose('Plan', [what, 'Add the part to look.md', 'Test it in happy-dom']) },
    { block: 'needs_deep_research', content: '', value: deepResearch ? 'yes' : 'no' },
  ]);
  if (deepResearch) h.ran('deep_research', [{ block: 'solution', content: prose('Solution', ['One table, sticky heads']) }]);
  h.ran('implement', [
    { block: 'code', content: 'commit 4be21c0: the change, with its tests' },
    { block: 'answer_review', content: 'Nothing to answer yet.' },
  ], { minutes: 9 });
}

/// The review sending the implementation round, `times` times, each with an opinion.
function goRound(h: History, times: number) {
  for (let i = 0; i < times; i += 1) {
    h.ran('review', [
      { block: 'opinions_implement', content: `Finding ${i + 1}: the totals are cut at narrow widths.` },
      { block: 'content', content: '', value: 'no' },
    ]);
    h.ran('implement', [
      { block: 'code', content: `commit ${(0x5a100 + i).toString(16)}: answers finding ${i + 1}` },
      { block: 'answer_review', content: `Finding ${i + 1}: fixed; the totals wrap.` },
    ], { minutes: 6 });
  }
}

interface Built {
  page: WorkPage;
  history: History;
}

/// What the plan's second round of the long history was told, and what came back of it: as a real round's, its closing
/// words are its structured return, which the interface gives once.
const PLAN_TOLD: ToldPage = {
  tries: [
    {
      message: [
        '# Plan (cell `plan`), round 2',
        '',
        '## Results of your calls',
        '',
        '- call 1, `read` `{"block":"request"}`: Show what each round of a work cost…',
        "- call 2, `read` `{\"path\":\"client/look.md\"}`: # The page's look…",
      ].join('\n'),
      cut: null,
      back: {
        kind: 'returned',
        text: null,
        structured: JSON.stringify(
          {
            asks: [],
            hands_in: [
              { block: 'needs_deep_research', content: '', value: 'yes' },
              { block: 'plan', content: '# Plan\n\n- A column of cost in the list\n- …', matters: true },
            ],
          },
          null,
          2,
        ),
        turns: 4,
        durationMs: 118_000,
        models: ['claude-haiku-5-5'],
      },
      ran: { level: '1', model: 'claude-haiku-5-5', effort: 'high' },
    },
  ],
  leftOut: 0,
};

function long(start: number): Built {
  const h = new History(start);
  h.wrote(
    'request',
    'person',
    REQUEST_TEXT('Show what each round of a work cost, in dollars and tokens, on the works page.'),
  );
  // The plan reads the item through the engine's tool, then hands in.
  const plan = h.start('plan');
  h.round(plan, {
    minutes: 2,
    asks: [
      { kind: 'tool', name: 'read', input: '{"block":"request"}', result: 'Show what each round of a work cost…' },
      { kind: 'tool', name: 'read', input: '{"path":"client/look.md"}', result: '# The page\'s look\n\nThe owner\'s page is an instrument panel…' },
    ],
  });
  const handed = h.round(plan, {
    hands: [
      {
        block: 'plan',
        content: prose('Plan', ['A column of cost in the list', 'Each round\'s cost in the step panel', 'Totals in the work\'s head']),
      },
      { block: 'needs_deep_research', content: '', value: 'yes' },
    ],
  });
  h.told.set(`${plan.run}/${handed.number}`, PLAN_TOLD);
  h.finish(plan);
  // The deep research hands in a block it may not write, and is sent back.
  const deep = h.start('deep_research');
  h.round(deep, {
    minutes: 4,
    hands: [
      { block: 'solution', content: prose('Solution', ['Dollars to the cent', 'Tokens in thousands']) },
      { block: 'code', content: 'a sketch of the column' },
    ],
    sentBack: [{ kind: 'notAnOutput', block: 'code' }],
  });
  h.round(deep, { minutes: 2, hands: [{ block: 'solution', content: prose('Solution', ['Dollars to the cent', 'Tokens in thousands']) }] });
  h.finish(deep);
  // The implementation asks the owner a question and rests at it...
  const first = h.start('implement');
  h.round(first, {
    minutes: 6,
    asks: [{ kind: 'tool', name: 'shell', input: 'npm run typecheck', result: 'Found 0 errors.' }],
  });
  h.round(first, {
    minutes: 1,
    asks: [{ kind: 'question', who: 'owner', question: 'Should the cached tokens be shown apart from the others?' }],
  });
  // ...meanwhile the owner writes an opinion for the plan, which runs again first: what it changes makes the
  // implementation's run stale, and it is abandoned.
  h.wrote('opinions_plan', 'person', 'The plan must say where the totals of a whole item are shown too.', null, 20);
  h.ran('plan', [
    {
      block: 'plan',
      content: prose('Plan', ['A column of cost in the list', 'Each round\'s cost in the step panel', 'Totals in the work\'s head and the item\'s']),
    },
    { block: 'needs_deep_research', content: '', value: 'yes' },
  ]);
  h.abandon(first);
  h.ran('deep_research', [
    { block: 'solution', content: prose('Solution', ['Dollars to the cent', 'Tokens in thousands', 'The item\'s totals']), matters: false },
  ]);
  const impl = h.start('implement');
  h.round(impl, {
    minutes: 12,
    asks: [
      { kind: 'tool', name: 'shell', input: 'npm run lint', result: '0 problems' },
      { kind: 'tool', name: 'shell', input: 'npm run build', result: 'built in 1.9s' },
    ],
  });
  h.round(impl, {
    minutes: 3,
    hands: [
      { block: 'code', content: 'commit 7c3e1a9: the cost column and the step panel\'s cost' },
      { block: 'answer_review', content: 'Nothing to answer yet.' },
    ],
  });
  h.finish(impl);
  // The review sends the implementation round twice, then is content.
  goRound(h, 2);
  h.ran('review', [{ block: 'content', content: '', value: 'yes' }]);
  // The outside writes the latest main; the check takes it in.
  h.wrote('main', 'outside', 'main at 3f2a91c', null, 30);
  h.ran('check', [{ block: 'ci', content: 'The full CI passed with the latest main taken in.', value: 'green' }], {
    minutes: 14,
    usd: 0,
  });
  const page = h.page(41, 141, 'Show the cost of each round on the works page', {
    kind: 'waits',
    at: 'merge',
    on: [{ kind: 'precondition', test: 'approved', words: 'The owner approved the merge', by: [{ kind: 'owner' }] }],
  });
  return { page, history: h };
}

/// Every work of the fixtures, by its number, with what each item's works are.
function build(now: number): { works: Map<number, Built>; items: Item[] } {
  const hours = (n: number) => now - n * 60 * MINUTE;
  const works = new Map<number, Built>();
  const items: Item[] = [];
  const keep = (built: Built) => {
    works.set(built.page.summary.number, built);
    const s = built.page.summary;
    const { title, level, history } = built.page;
    items.push({ number: s.item, title, level, added: s.added, dependsOn: [], history, work: s });
  };

  keep(long(hours(30)));

  {
    // A round under way.
    const h = new History(hours(1));
    h.wrote('request', 'person', REQUEST_TEXT('Keep the panel\'s width when the page is loaded again.'));
    h.ran('plan', [
      { block: 'plan', content: prose('Plan', ['Keep the width in the browser']) },
      { block: 'needs_deep_research', content: '', value: 'no' },
    ]);
    const run = h.start('implement');
    h.round(run, { minutes: 7, asks: [{ kind: 'tool', name: 'shell', input: 'npm run typecheck', result: 'Found 0 errors.' }] });
    const r = h.round(run, { underWay: true });
    keep({ page: h.page(42, 142, 'Keep the panel\'s width', { kind: 'round', cell: 'implement', run: run.run, round: r.number, since: r.began }), history: h });
  }

  {
    // Waiting for the owner's answer to a question.
    const h = new History(hours(5));
    h.wrote('request', 'person', REQUEST_TEXT('List the works of every item, those that wait for the owner first.'));
    const run = h.start('plan');
    h.round(run, {
      asks: [{ kind: 'question', who: 'owner', question: 'Should the list show the works of cancelled items, or leave them out?' }],
    });
    const call = run.calls[0].call;
    keep({
      page: h.page(43, 143, 'List every work', {
        kind: 'waits',
        at: 'plan',
        on: [{ kind: 'call', call, who: 'owner', what: run.calls[0].what }],
      }),
      history: h,
    });
  }

  {
    // A run at its limit of returns sent back.
    const h = new History(hours(8));
    h.wrote('request', 'person', REQUEST_TEXT('Say a choice\'s values after its name.'));
    h.ran('plan', [
      { block: 'plan', content: prose('Plan', ['The values after the name, muted']) },
      { block: 'needs_deep_research', content: '', value: 'no' },
    ]);
    const run = h.start('implement');
    h.round(run, {
      hands: [{ block: 'code', content: 'commit 1a2b3c4' }, { block: 'code', content: 'commit 1a2b3c5' }],
      sentBack: [{ kind: 'writtenTwice', block: 'code' }],
    });
    h.round(run, {
      hands: [{ block: 'code', content: 'commit 1a2b3c6' }, { block: 'answer_review', content: 'x', value: 'done' }],
      sentBack: [{ kind: 'notAChoice', block: 'answer_review' }],
    });
    h.round(run, { hands: [{ block: 'solution', content: 'a solution' }] });
    const limit = { kind: 'atLimit' as const, limit: { of: 'returns' as const, sentBack: 2 } };
    const call = h.atLimit(run, limit);
    keep({ page: h.page(44, 144, 'Say a choice\'s values', { kind: 'waits', at: 'implement', on: [{ kind: 'call', call, who: 'owner', what: limit }] }), history: h });
  }

  {
    // A cell at its limit of ways round.
    const h = new History(hours(12));
    upToReview(h, 'Draw the way back in its purple.', false);
    goRound(h, 3);
    h.ran('review', [{ block: 'opinions_implement', content: 'Finding 4: the arrow points the wrong way.' }, { block: 'content', content: '', value: 'no' }]);
    keep({ page: h.page(45, 145, 'Draw the way back', { kind: 'waits', at: 'implement', on: [{ kind: 'limit', runs: 3 }] }), history: h });
  }

  {
    // Paused.
    const h = new History(hours(20));
    h.wrote('request', 'person', REQUEST_TEXT('Fold the panel away with Escape.'));
    h.ran('plan', [
      { block: 'plan', content: prose('Plan', ['Escape lets go of what is chosen']) },
      { block: 'needs_deep_research', content: '', value: 'no' },
    ]);
    h.did('paused', 30);
    h.did('resumed', 60);
    h.did('paused', 10);
    keep({ page: h.page(46, 146, 'Fold the panel away', { kind: 'paused', by: 'owner', why: null }, true), history: h });
  }

  {
    // Paused by the engine: the implementation's round given the result of its tool failed, and is given again once
    // the owner lets the work on.
    const h = new History(hours(3));
    h.wrote('request', 'person', REQUEST_TEXT('Say why a work is paused.'));
    h.ran('plan', [
      { block: 'plan', content: prose('Plan', ['The reason beside the pause']) },
      { block: 'needs_deep_research', content: '', value: 'no' },
    ]);
    const run = h.start('implement');
    h.round(run, { minutes: 5, asks: [{ kind: 'tool', name: 'shell', input: 'npm test', result: '212 passed' }] });
    const why = "The agent's program exited with status 1: the session could not be resumed.";
    h.round(run, { minutes: 0.5, usd: 0.01, fails: why });
    keep({ page: h.page(50, 150, 'Say why a work is paused', { kind: 'paused', by: 'engine', why }, true), history: h });
  }

  {
    // Merged.
    const h = new History(hours(50));
    upToReview(h, 'Show a block\'s signs.', true);
    h.ran('review', [{ block: 'content', content: '', value: 'yes' }]);
    h.wrote('main', 'outside', 'main at 9e8d7c6', null, 20);
    h.ran('check', [{ block: 'ci', content: 'Passed.', value: 'green' }], { minutes: 12, usd: 0 });
    h.wrote('approval', 'person', 'Approved.', 'yes', 40);
    h.ran('merge', [], { minutes: 1, usd: 0 });
    keep({ page: h.page(47, 147, 'Show a block\'s signs', { kind: 'merged' }), history: h });
  }

  {
    // Cancelled, after a let-go at the limit of ways round.
    const h = new History(hours(70));
    upToReview(h, 'Draw the chart in a canvas.', false);
    goRound(h, 3);
    h.ran('review', [{ block: 'opinions_implement', content: 'Finding 4: still slow.' }, { block: 'content', content: '', value: 'no' }]);
    h.letGo('implement', 120);
    const run = h.start('implement');
    h.round(run, { underWay: true });
    h.did('cancelled', 15);
    h.abandon(run);
    keep({ page: h.page(48, 148, 'Draw the chart in a canvas', { kind: 'cancelled' }), history: h });
  }

  {
    // A fault in the records.
    const h = new History(hours(26));
    h.wrote('request', 'person', REQUEST_TEXT('Read the flow again on a change.'));
    h.ran('plan', [
      { block: 'plan', content: prose('Plan', ['Read again on the event']) },
      { block: 'needs_deep_research', content: '', value: 'no' },
    ]);
    keep({ page: h.page(49, 149, 'Read the flow again', { kind: 'fault', words: 'Two entries of the work at place 3' }), history: h });
  }

  return { works, items };
}

let built: ReturnType<typeof build> | undefined;

function fixtures() {
  built ??= build(Math.floor(Date.now() / MINUTE) * MINUTE);
  return built;
}

function work(n: number): Built {
  const w = fixtures().works.get(n);
  if (!w) throw new Error(`/api/v1/works/${n} answered 404: no such work`);
  return w;
}

export function fixtureItems(): Items {
  return { items: fixtures().items, outage: null };
}

/// The copies of the database as they stand when all is well: the newest taken three hours ago and kept off the machine.
export function fixtureCopies(): Copies {
  const now = Date.now();
  const taken = now - 3 * 60 * 60 * 1000;
  return {
    state: { kind: 'ok' },
    now,
    lastTaken: taken,
    lastUploaded: taken + 60 * 1000,
    pending: 0,
    lastFailure: null,
  };
}

export function fixtureWork(n: number): WorkPage {
  return work(n).page;
}

export function fixtureVersion(n: number, block: string, at: number): VersionPage {
  const content = work(n).history.contents.get(`${block}@${at}`);
  if (content === undefined) throw new Error(`no version ${block}@${at}`);
  return { block, at, content };
}

export function fixtureRound(n: number, run: number, round: number): RoundPage {
  const page = work(n).history.rounds.get(`${run}/${round}`);
  if (!page) throw new Error(`no round ${round} of run ${run}`);
  return page;
}

export function fixtureTold(n: number, run: number, round: number): ToldPage {
  const history = work(n).history;
  if (!history.rounds.has(`${run}/${round}`)) throw new Error(`no round ${round} of run ${run}`);
  return history.told.get(`${run}/${round}`) ?? { tries: [], leftOut: 0 };
}

export function fixtureCall(n: number, call: number): CallPage {
  const page = work(n).history.callPages.get(call);
  if (!page) throw new Error(`no call ${call}`);
  return page;
}

/// The runs of the long history, for the tests to find their parts by.
export function longHistory(): WorkPage {
  return fixtureWork(41);
}

export type { Run };
