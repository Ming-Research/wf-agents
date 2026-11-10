// Made-up contents for the console, for what the new engine does not give yet: every value here that the drawing
// shows is mock, and each is replaced by the engine's own once it gives it, without changing the drawing code. Brought
// over from the console of the engine on `main` (`client/src/console/window-mock.ts`), its bodies as they were.
//
// Mock, field by field:
// - On a card and in the window: `unseen` (the item is new to the owner, and the passages changed since they read it),
//   `unread` (messages from others the owner has not read), and marking the item read (kept in this page only).
// - In the window's head and item line: the item's `project`, its `tier`, and whether it is a `goal`.
// - In the window's work tab: the work's `tier`.
// - In the item tab: the body (Background, Problem, Plan), the plan's choices and its Confirm (kept in this page only),
//   and the notes written before (who wrote them, to whom, and the replies).
// Real, from the engine: everything else, such as the item's standing, when it was added and what it waits for; the
// work's state, cells, rounds, cost, start, time bar and returns sent back; what it waits at; and the notes the owner
// sends, which are written into a block of the work.

import type { Item } from '../works-api';
import type { Body, Note, Part } from './window';

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

/// The messages of item 263's work, in English, as notes.
const notes263 = (now: number): Note[] => [
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

/// The bodies that show the window's parts, by what an item's work does (`richOf`).
const RICH: { starts: string; make: (now: number) => Partial<Body> }[] = [
  {
    starts: 'Watch for anomalies and settle them',
    make: (now) => ({ ...ITEM_263, notes: notes263(now) }),
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
      notes: [
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
  project: string;
  /// Its tier, none where it has none.
  tier: string | null;
};

export function itemMock(item: Item): ItemMock {
  const open = item.work.now.kind !== 'merged' && item.work.now.kind !== 'cancelled';
  return {
    unseen: open && item.number % 4 === 1 && !read.has(item.number),
    unread: open && item.number % 5 === 2 ? 2 : 0,
    project: 'wf-agents',
    tier: '2',
  };
}

/// Whether an item is a goal: one that has no work of its own going on and items under it.
export const goalMock = (workDrawn: boolean, children: number): boolean => !workDrawn && children > 0;

/// The work's tier.
export const workTierMock = (): string => '2 (Sonnet)';

/// Which of the bodies above an item takes: one that waits for the owner, the plans with choices; one running, the one
/// with notes; one that waits for another, the one whose plan waits; any other, a short one.
function richOf(item: Item): string | null {
  const now = item.work.now;
  if (now.kind === 'waits' && now.on.some((w) => w.kind === 'limit' || w.who === 'owner'))
    return item.number % 2 === 0 ? 'Watch for anomalies and settle them' : 'New, and waiting for your answer';
  if (now.kind === 'waits') return 'Waiting for your ruling';
  if (now.kind === 'round' || now.kind === 'next') return 'Running for 12 minutes';
  return null;
}

/// The item's body and notes, made up.
export function bodyMock(item: Item): Body {
  const now = Date.now();
  const which = richOf(item);
  const rich = RICH.find((r) => r.starts === which)?.make(now) ?? {};
  const plain = (text: string): Part[] => [{ kind: 'text', text }];
  const body: Body = {
    background: plain('Made by the architect from a matter it was handling.'),
    problem: plain('What the item fixes or adds, in a few sentences.'),
    plan: plain('How it is to be done, once researched.'),
    planWaits: false,
    notes: [],
    ...rich,
  };
  if (!read.has(item.number) && itemMock(item).unseen) return body;
  // Read, or never new: no passage is marked changed.
  const clear = (ps: Part[]) => ps.map((p) => ({ ...p, changed: false }));
  return { ...body, background: clear(body.background), problem: clear(body.problem), plan: clear(body.plan) };
}
