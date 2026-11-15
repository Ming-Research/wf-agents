// Made-up contents for the console, for what the new engine does not give yet: every value here that the drawing
// shows is mock, and each is replaced by the engine's own once it gives it, without changing the drawing code. Brought
// over from the console of the engine on `old-main` (`client/src/console/window-mock.ts`), its bodies as they were.
//
// Mock, field by field:
// - On a card: `openQuestions`, the questions that wait for the owner's answer and do not stop the work yet (the
//   engine says only what a work is stopped for).
// - On a card and in the window: `unseen` (the item is new to the owner, and the passages changed since they read it),
//   `unread` (messages from others the owner has not read), and marking the item read (kept in this page only).
// - In the window's item line: the item's `tier`, and whether it is a `goal`.
// - In the window's work tab: the work's `tier`, and the `verdicts` of its judges (item 262 only).
// - In the item tab: the body (Background, Problem, Plan), the plan's choices, which of them the owner
//   already confirmed, and its Confirm (kept in this page only), and the messages written before (who wrote them, to whom,
//   what they quote, and the replies). Item 262 has a whole made-up content of its own; the others take one of the
//   bodies below by what their work does, or a short one.
// - In the plan, a question a cell asked while working: its options and the one recommended (`askedMock`). The engine
//   gives only the question's sentence; the label of the option the owner confirms is sent to it as the answer.
// Real, from the engine: everything else, such as the item's standing, when it was added and what it waits for; the
// work's state, cells, rounds, cost, start, time bar and returns sent back; what it waits at; and the messages the owner
// sends, which are written into a block of the work.

import type { Item } from '../works-api';
import { type Body, type Choice, type Message, type Part, quoteOf } from './window';

const HOUR = 3600000;

const DIAGRAM = `\`\`\`mermaid
flowchart LR
  W[Work runs] --> M{Health numbers past their line?}
  M -- no --> W
  M -- yes --> A[Anomaly recorded]
  A --> E[Ending chosen: retry, pause or end]
\`\`\``;

/// Item 263 of the trial board, in English: where it came from, its problem, the plan its research wrote with the
/// decision points it left where each belongs, and the messages of its work.
const ITEM_263_PLAN = `\`\`\`mermaid
flowchart TD
  B["Hourly beat samples the numbers"] --> L["Line from the number's own last 7 days"]
  L --> C{"Point past the line?"}
  C -- no --> B
  C -- yes --> R["Recorded in anomalies, one row per point"]
  R --> K{"Kind already open or reported?"}
  K -- yes --> N["Counted only"]
  K -- no --> S{"Layer"}
  S -- system --> F["Notice, forwarded by forward_faults"]
  S -- project --> A["Report in the project's architect queue"]
  A --> O["Architect records or merges an observation: the ending"]
\`\`\``;

const ITEM_263: Partial<Body> = {
  background: [
    {
      kind: 'text',
      text:
        'Split off from #231, which recorded the health numbers and left watching them to this item. The owner asked ' +
        "for it in matters 491 and 628: problems are found by chance, and the system's and each project's must be kept apart.",
    },
  ],
  problem: [
    {
      kind: 'text',
      text:
        'Feature. Anomalies are found only by chance: no number has a line drawn from its own history, crossing it is ' +
        'neither recorded nor classed, nothing gives it an ending, and nothing sends it to whoever answers for its layer. ' +
        'Of the 7 problems of 2 November, 4 were seen by the owner by chance.',
    },
  ],
  plan: [
    {
      kind: 'text',
      text:
        'Every hour, after the health numbers are sampled, each watched number is compared with its line; a point past it ' +
        'is recorded as an anomaly of its kind, and sent once per kind to whoever answers for its layer.\n\n' +
        ITEM_263_PLAN,
    },
    {
      kind: 'choice',
      question: 'Which rule draws the line? One rule for the whole system; #242 will use it later.',
      options: [
        {
          label: 'Median + 3 × 1.4826 × MAD',
          detail:
            "The last 7 days, filled up to 20 values. Built from each number's own history, and one extreme value does not " +
            'raise it; the constants are part of the rule, not taken from the data.',
        },
        {
          label: 'Mean + 3 standard deviations',
          detail:
            "The item's own example: common and easy to read, but one extreme value pulls the line up for the next.",
        },
        {
          label: "The run guard's 2 × max − median",
          detail: "Reuses the guard's rule, which is known to stop nothing: all 13 of its rows are only noted.",
        },
      ],
      recommended: 0,
      chosen: null,
    },
    {
      kind: 'text',
      text:
        'Watched at first: time to merge, spend per hour, progress (a new sample: the oldest unfinished work of each ' +
        'project), returns sent back, queue backlog, and, for projects, the wait for CI.',
      changed: true,
    },
    {
      kind: 'choice',
      question: "When one of the system's own numbers (queue backlog, return rate) crosses its line, who gets it?",
      options: [
        {
          label: 'forward_faults',
          detail:
            "A fault of wf-agents: a notice the owner's setting forwards to a project's architect, left with the owner while " +
            'unset. No new rule, and the owner can change where it goes at any time.',
        },
        {
          label: 'wf_agents_project()',
          detail:
            'The project matched by repository gets it in its queue: handled without a setting, but never redirected.',
        },
        {
          label: 'A new install setting',
          detail: 'Explicit, but a third way of naming the wf-agents project, which may disagree with the other two.',
        },
      ],
      recommended: 0,
      chosen: null,
    },
    {
      kind: 'text',
      text:
        'Each kind keeps one thread: while an observation of the kind is open, or a report waits, a new point only ' +
        "counts. A kind that comes back after its fix merged is marked as recurring and sent again. The architect's check " +
        'refuses to finish an anomaly report without recording or merging an observation, so each kind ends one of three ' +
        'ways: an item to fix it, accepted with its reason, or a new check.',
    },
    {
      kind: 'choice',
      question:
        "When do the run guard (#193, the held #242) and #236 move onto these records? Until then the guard's records " +
        'and these stand side by side.',
      options: [
        {
          label: 'Item by item',
          detail: 'This item leaves the line and the records ready, and #242 moves the guard onto them: a small scope.',
        },
        {
          label: 'Here, with this item',
          detail:
            'One set of records sooner, but the guard stops rounds as they run and #242 is held: the scope runs away.',
        },
      ],
      recommended: 0,
      chosen: null,
    },
    {
      kind: 'text',
      text:
        "What changes: the health crate (line, watch, record), a migration with the table `anomalies`, the engine's hourly " +
        "beat, one architect check, the text of cell H1, and the fault's words on the page.",
    },
  ],
  planWaits: true,
};

/// The messages of item 263's work, in English.
const messages263 = (now: number): Message[] => [
  {
    who: 'engineer',
    at: now - 7.5 * HOUR,
    to: ['owner'],
    quote: null,
    text:
      "This work changes flow.json (the text of the architect's cell H1, and one check in engine.checks), so its merge " +
      "waits for your approval at M1; the pull request will link the flow page's comparison.",
  },
  {
    who: 'engineer',
    at: now - 7.4 * HOUR,
    to: ['owner'],
    quote: null,
    text:
      "You never answered three points of W275's design: who gets the system's alerts, the line's rule, and when the " +
      "guard moves over. I follow the design's recommendations, and leave them to you as decision points; please answer " +
      'before the merge.',
  },
  {
    who: 'architect',
    at: now - 6 * HOUR,
    to: ['engineer'],
    quote: null,
    text:
      'About your side report at R1 (the page shows every fault but flow_anomaly as a cache miss): confirmed in ' +
      '`Messages.tsx` line 744. #263 fixes it, with no item of its own: when you add the anomaly fault, look its words up by ' +
      'its kind, and add them to `messages.ts`.',
  },
];

/// Item 262 of the demo data, whole, as its agents would write it for the owner: its work went through the deep
/// research and the plan's review, was sent back by the review, and waits at the implementation; one question of the
/// plan the owner confirmed, one is still open, and the messages discuss it. All of it is made up.
const ITEM_262_OPEN: Extract<Part, { kind: 'choice' }> = {
  kind: 'choice',
  question:
    "Is a failed try part of a cell's cost? `work.cost` is defined as what a cell spent to hand in its blocks, and a " +
    'failed try handed in nothing.',
  options: [
    {
      label: 'Yes: into work.cost, its reason kept as failed',
      detail:
        'The cost by cell is whole in one place, and its sum equals what was billed. Cost: the definition changes from ' +
        '"spent to hand in" to "spent trying", and whoever reads it sees one reason more.',
    },
    {
      label: 'A number of its own, work.failed_cost',
      detail: 'work.cost keeps its definition. Cost: to see all a work spent, two numbers are added, and the System page gains a chart.',
    },
    {
      label: 'Only the tries that ran past one minute',
      detail:
        'Leaves out the tries that failed at once. Cost: a constant to choose and explain; the short tries drop out of the ' +
        'sum, and part of the gap comes back.',
    },
  ],
  recommended: 0,
  chosen: null,
};

const ITEM_262: Partial<Body> = {
  background: [
    {
      kind: 'text',
      text:
        'Since 4 March each try of a round is kept: the table `round_tries`, one row for each try, with `started`, ' +
        '`ended`, what it cost and how it ended (`returned`, `failed`, `timed_out`).',
    },
    {
      kind: 'text',
      text:
        "The work's cost by cell counts only the try that returned. Item #118, which added the cost by cell, counts the " +
        'tries that `returned` and says that the others are left to this item.',
    },
    {
      kind: 'text',
      text: 'With the failed tries left out, a work that fails often looks cheap, and the owner cannot tell which works cost the most.',
    },
  ],
  problem: [
    {
      kind: 'text',
      text:
        'Bug. `work.cost` silently leaves out every try that failed: the cost summed by cell is less than what was ' +
        'billed, and the gap belongs to no cell. Demo data, one whole day, three slots:',
    },
    {
      kind: 'text',
      text:
        '```\n' +
        'work.cost by cell        R1          $4.10\n' +
        '                         I1          $9.30\n' +
        '                         V1          $2.20\n' +
        '                         sum        $15.60\n' +
        'billed                              $21.90\n' +
        'round_tries, 37 rows     no cell     $6.30\n' +
        '```',
    },
    {
      kind: 'text',
      text:
        'Cause: `totals::cost` reads only the tries that `returned`, and never the others; `failed` is named only in ' +
        "the number's `missing` (`FAILED_UNCOUNTED`).",
    },
  ],
  plan: [
    {
      kind: 'text',
      text:
        '`totals::cost` reads every row of `round_tries` that falls in the period, one amount for each, and a try still ' +
        'open counts what it has used so far. A try belongs to one round only, so nothing is counted twice.',
    },
    ITEM_262_OPEN,
    {
      kind: 'text',
      text: "The number's `missing` drops `failed`, and `FAILED_UNCOUNTED` is deleted with it.",
    },
    {
      kind: 'choice',
      question: 'Before 4 March (migration 0042) there is no round_tries: how is the cost of a failed try before then counted?',
      options: [
        {
          label: 'Marked unmeasured, with the day it starts',
          detail: 'Keeps missing, with a start date; the chart shows that stretch empty rather than zero. Cost: missing must carry a date.',
        },
        {
          label: "Worked out from the provider's bills",
          detail: 'Gives the past numbers. Cost: a bill covers a day, not a try, so the split by cell would be a guess.',
        },
        {
          label: 'Counted as zero',
          detail: 'The least work. Cost: the chart looks as if failed tries began on 4 March.',
        },
      ],
      recommended: 0,
      chosen: 0,
      confirmed: true,
    },
    {
      kind: 'text',
      text:
        'Tests go in `crates/health/src/tests.rs`: a try inside the period, one across its start, one still open, and a ' +
        'round tried three times counted once for each try. Changes `totals.rs`, `lib.rs` and `tests.rs`; the schema stays.',
    },
    {
      kind: 'text',
      text:
        'A try still open counts what it has used so far, as an open round does, or two ways of counting in one number ' +
        'would keep the sum from matching. A try that timed out counts as usual: how it ended does not change what it ' +
        'spent. No index on round_tries: a few dozen rows a day, and a scan by `started` takes 0.3 ms on the demo data.',
    },
  ],
  planWaits: true,
};

/// The messages of item 262's work, in time order: the owner asks about the open question, the deep research that wrote
/// the plan answers, and the implementation answers one about what it decided.
const messages262 = (now: number): Message[] => [
  {
    who: 'owner',
    at: now - 5 * HOUR,
    to: ['D1'],
    quote: quoteOf(ITEM_262_OPEN),
    text: 'Explain why the first is recommended. I do not see what the second one changes.',
  },
  {
    who: 'D1',
    at: now - 4.6 * HOUR,
    to: ['owner'],
    quote: null,
    text:
      'The second changes no number there is, it only adds one: `work.cost` still counts the tries that returned, and ' +
      'the failed ones go into `work.failed_cost`. The cost falls on whoever reads them: to know what a work spent, the ' +
      'two are added, and the System page needs one chart more.\n\n' +
      'The first is recommended because the gap is what this item is for. In one number, the sum by cell equals what ' +
      'was billed ($21.90 in the example above), and the gap is plainly gone.',
  },
  {
    who: 'owner',
    at: now - 4.2 * HOUR,
    to: ['D1'],
    quote: null,
    text: 'Once the definition changes, will anything written against `work.cost` raise a false alarm?',
  },
  {
    who: 'D1',
    at: now - 3.9 * HOUR,
    to: ['owner'],
    quote: null,
    text:
      "No. Only the System page and the architect's numbers read the two, both split by cell, with no threshold on the " +
      'sum; a `failed` reason adds one row. I looked through the rules: none judges by the sum of `work.cost`.',
  },
  {
    who: 'owner',
    at: now - 2 * HOUR,
    to: ['I1'],
    quote: 'a try still open counts what it has used so far.',
    text: 'What of the tries left open when the engine restarts? Do they keep growing?',
  },
  {
    who: 'I1',
    at: now - 1.6 * HOUR,
    to: ['owner'],
    quote: null,
    text:
      'They do not keep growing: the new engine closes or takes up the open tries in its first round. Between the two ' +
      'rounds a few seconds to a minute more is counted. I added a test: one try before the restart and one after, and ' +
      'the sum equals the two.',
  },
];

/// The verdicts of item 262's judges, as far as its work has come.
const VERDICTS_262: [string, string][] = [
  ['Review the plan', 'Settled'],
  ['Review', 'Sent back to Implement, 1 finding'],
  ['Adversarial testing', 'Skipped'],
  ['Check before the merge', 'Not yet'],
];

const is262 = (item: Item) => item.number === 262;

/// The bodies that show the window's parts, by what an item's work does (`richOf`).
const RICH: { starts: string; make: (now: number) => Partial<Body> }[] = [
  {
    starts: 'Watch for anomalies and settle them',
    make: (now) => ({ ...ITEM_263, messages: messages263(now) }),
  },
  {
    starts: 'New, and waiting for your answer',
    make: () => ({
      background: [
        {
          kind: 'text',
          text: 'Found while trying the flow on the trial board: three long rounds went on for hours and nobody noticed.',
        },
      ],
      problem: [
        {
          kind: 'text',
          text: 'Feature. A work that runs past its health numbers is not seen by anyone and has no ending.',
          changed: true,
        },
      ],
      plan: [
        {
          kind: 'text',
          text: "Watch each work's health numbers after every round, and give each anomaly an ending:\n\n" + DIAGRAM,
        },
        {
          kind: 'choice',
          question: "Where does each health number's line come from?",
          options: [
            {
              label: 'Fixed lines',
              detail: 'One value per number in the configuration. Simple, but wrong for unusual projects.',
            },
            {
              label: "From the project's own history",
              detail: 'The line is the 95th percentile of the last 50 works. Needs a history first.',
            },
          ],
          recommended: 1,
          chosen: null,
        },
        {
          kind: 'text',
          text: "Then the ending is chosen by the anomaly's kind: a round too long is retried once, a work sent back too often is paused for the owner.",
          changed: true,
        },
      ],
      planWaits: true,
    }),
  },
  {
    starts: 'Waiting for your ruling',
    make: () => ({
      background: [
        { kind: 'text', text: 'Reported by the architect while merging two items that changed the same page.' },
      ],
      problem: [{ kind: 'text', text: 'Bug. The page draws stale counts after a work ends until it is reloaded.' }],
      plan: [
        {
          kind: 'text',
          text: 'Send the counts with the event that ends a work, and drop the timer that reads them again. Two files change: `live.ts` and the work list.',
        },
      ],
      planWaits: true,
    }),
  },
  {
    starts: 'Running for 12 minutes',
    make: (now) => ({
      background: [{ kind: 'text', text: 'Asked for by the owner in the discussion of the console.' }],
      problem: [{ kind: 'text', text: 'Feature. The console shows no prices or time zone.' }],
      plan: [{ kind: 'text', text: "Read prices from the configuration and show times in the owner's zone." }],
      messages: [
        {
          who: 'owner',
          at: now - 2 * HOUR,
          to: ['engineer'],
          quote: "show times in the owner's zone",
          text: "Use the zone set in the configuration, not the machine's.",
        },
        {
          who: 'engineer',
          at: now - 1.5 * HOUR,
          to: ['owner'],
          quote: null,
          text: "Done: times now follow `owner.timezone`, and the machine's zone is used only when it is not set.",
        },
      ],
    }),
  },
];

/// The items the owner marked read in this page.
const read = new Set<number>();

/// Marks the item read: it is no longer new to the owner, and none of its passages is marked changed.
export function markRead(item: number) {
  read.add(item);
}

/// What the console shows of an item that the engine does not give.
export type ItemMock = {
  /// New to the owner, or changed since they last read it.
  unseen: boolean;
  /// Messages from others in its work's conversations that the owner has not read.
  unread: number;
  /// Questions that wait for the owner's answer and do not stop its work yet: the work goes on.
  openQuestions: number;
  /// Its tier, none where it has none.
  tier: string | null;
  /// The verdicts of its work's judges so far, each a row of the work's facts: the judge, and what it said.
  verdicts: [string, string][];
};

export function itemMock(item: Item): ItemMock {
  const open = item.work.now.kind !== 'merged' && item.work.now.kind !== 'cancelled';
  return {
    unseen: open && item.number % 4 === 1 && !read.has(item.number),
    // How many things are new: some wherever the item is new to the owner, none elsewhere, so that the line and the
    // number always go together.
    unread: open && item.number % 4 === 1 && !read.has(item.number) ? 1 + (item.number % 3) : 0,
    openQuestions: (item.work.now.kind === 'round' || item.work.now.kind === 'next') && item.number % 3 !== 0 ? 1 : 0,
    tier: '2',
    verdicts: is262(item) ? VERDICTS_262 : [],
  };
}

/// Whether an item is a goal: one that has no work of its own going on and items under it.
export const goalMock = (workDrawn: boolean, children: number): boolean => !workDrawn && children > 0;

/// A question a cell asked while working, as a choice at the plan's end: the engine's sentence, with made-up options
/// that fit a question answered yes or no.
export const askedMock = (question: string, call: number, cell: string): Choice => ({
  kind: 'choice',
  question,
  options: [
    { label: 'Yes', detail: 'Go on as the question proposes.' },
    { label: 'No', detail: 'Go on the other way the question names.' },
  ],
  recommended: 0,
  chosen: null,
  asks: { call, cell },
});

/// The work's tier.
export const workTierMock = (): string => '2 (Sonnet)';

/// Which of the bodies above an item takes: one that waits for the owner, the plans with choices; one running, the one
/// with messages; one that waits for another, the one whose plan waits; any other, a short one.
function richOf(item: Item): string | null {
  const now = item.work.now;
  if (
    now.kind === 'waits' &&
    now.on.some(
      (w) =>
        w.kind === 'limit' ||
        (w.kind === 'call' ? w.who === 'owner' : w.kind === 'precondition' && w.by.some((m) => m.kind === 'owner')),
    )
  )
    return item.number % 2 === 0 ? 'Watch for anomalies and settle them' : 'New, and waiting for your answer';
  if (now.kind === 'waits') return 'Waiting for your ruling';
  if (now.kind === 'round' || now.kind === 'next') return 'Running for 12 minutes';
  return null;
}

/// The item's body and messages, made up.
export function bodyMock(item: Item): Body {
  const now = Date.now();
  const which = richOf(item);
  const rich = is262(item)
    ? { ...ITEM_262, messages: messages262(now) }
    : (RICH.find((r) => r.starts === which)?.make(now) ?? {});
  const plain = (text: string): Part[] => [{ kind: 'text', text }];
  const body: Body = {
    background: plain('Made by the architect from a matter it was handling.'),
    problem: plain('What the item fixes or adds, in a few sentences.'),
    plan: plain('How it is to be done, once researched.'),
    planWaits: false,
    messages: [],
    ...rich,
  };
  if (!read.has(item.number) && itemMock(item).unseen) return body;
  // Read, or never new: no passage is marked changed.
  const clear = <P extends Part>(ps: P[]): P[] => ps.map((p) => ({ ...p, changed: false }));
  return {
    ...body,
    background: clear(body.background),
    problem: clear(body.problem),
    plan: clear(body.plan),
  };
}
