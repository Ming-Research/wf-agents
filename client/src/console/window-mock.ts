// Made-up contents for the window beside the console, for trying it before the engine writes items this way: for any
// item drawn, its facts from the tree and a short body; for a few of the demo's items, a body that shows each thing the
// window draws (a choice in the middle of the plan, a plan waiting to be confirmed, a diagram, passages changed since
// the owner read them, notes and their replies). The first is a real item of the trial board, put into English and cut
// short as the window wants its parts: item 263, its research's decision points and its work's messages.

import type { Shown } from './items';
import type { Part, WindowItem } from './window';

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

const ITEM_263: Partial<WindowItem> = {
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
const notes263 = (now: number): WindowItem['notes'] => [
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

/// The bodies of the demo's items that show the window's parts, by the start of their titles.
const RICH: { starts: string; make: (now: number) => Partial<WindowItem> }[] = [
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

/// The window's contents for an item of the tree, made up.
export function windowMock(shown: Shown): WindowItem {
  const now = Date.now();
  const rich = RICH.find((r) => shown.title.startsWith(r.starts))?.make(now) ?? {};
  const plain = (text: string): Part[] => [{ kind: 'text', text }];
  const w = shown.work;
  return {
    id: shown.id,
    title: shown.title,
    item: {
      standing: shown.standing,
      goal: shown.work === null && shown.children.length > 0,
      project: 'wf-agents',
      level: '2',
      madeAt: now - 26 * HOUR,
      waitsFor: shown.blocked > 0 ? [263] : [],
      unseen: shown.unseen,
    },
    work:
      w === null
        ? null
        : {
            id: 300 + shown.id,
            state: w.state,
            waits: w.state === 'asking' ? 'your answer' : w.state === 'waiting' ? 'item 263 to merge' : null,
            stage: w.stage,
            stages: w.stages,
            from: w.from,
            rounds: 14,
            costUsd: 3.42,
            bar: w.bar,
            sentBack: w.sentBack,
            tier: '2 (Sonnet)',
          },
    background: plain('Made by the architect from a matter it was handling.'),
    problem: plain('What the item fixes or adds, in a few sentences.'),
    plan: plain('How it is to be done, once researched.'),
    planWaits: false,
    notes: [],
    ...rich,
  };
}
