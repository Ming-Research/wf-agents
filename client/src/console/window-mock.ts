// Made-up contents for the window beside the console, for trying it before the engine writes items this way: for any
// item drawn, its facts from the tree and a short body; for a few of the demo's items, a body that shows each thing the
// window draws (a choice in the middle of the plan, a plan waiting to be confirmed, a diagram, passages changed since
// the owner read them, notes and their replies).

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

/// The bodies of the demo's items that show the window's parts, by the start of their titles.
const RICH: { starts: string; make: (now: number) => Partial<WindowItem> }[] = [
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
      madeBy: 'architect',
      madeAt: now - 26 * HOUR,
      waitsFor: shown.blocked > 0 ? [263] : [],
      neededBy: shown.children.length,
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
