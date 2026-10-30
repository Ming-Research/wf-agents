// A message of a talk, as a chat draws it, one part wherever the page shows agents and the owner speaking to each other
// (a conversation of the messages view, a helper's thread with the engineer on a work's page): an agent's in the box
// of what an agent handed back, filled and edged in its side's display colour, beside its avatar; the owner's in the
// sheet of what is sent to an agent, on the right; and in a thread between two agents, the side across from the
// engineer on the right, its avatar after it. Its head (who, to whom, when, its round) over what it says, and under it
// what answers it there.

import { BugIcon, CodeReviewIcon, LightBulbIcon, ProjectRoadmapIcon, ToolsIcon } from '@primer/octicons-react';
import type { ReactNode } from 'react';

/// The colour of a side of a talk: the engineer's, the architect's, a helper's of a work (the advisor, the review, the
/// adversarial testing), or the owner's sheet.
export type Tone = 'engineer' | 'architect' | 'helper' | 'owner';

/// The colour of who says something, by its line: the architect's for the architect and its policy setting, a
/// helper's for a work's helpers, else the engineer's.
export function toneOf(who: string): Tone {
  if (who === 'owner') return 'owner';
  if (who === 'architect' || who.startsWith('architect')) return 'architect';
  if (who === 'advisor' || who === 'review' || who === 'adversarial') return 'helper';
  return 'engineer';
}

/// The mark of each helper of a work, by its line.
const HELPER_MARKS: Record<string, ReactNode> = {
  advisor: <LightBulbIcon />,
  review: <CodeReviewIcon />,
  adversarial: <BugIcon />,
};

/// An agent's avatar: its line's mark in its side's colour, the engineer's tools, the architect's plan, the advisor's
/// light, the review's and the adversarial testing's own.
export function Avatar({ who }: { who: string }) {
  const tone = toneOf(who);
  const mark = tone === 'architect' ? <ProjectRoadmapIcon /> : tone === 'helper' ? HELPER_MARKS[who] : <ToolsIcon />;
  return (
    <span className={`talk-avatar talk-avatar-${tone}`} aria-hidden="true">
      {mark}
    </span>
  );
}

/// One message: `who` says it (an agent's line, or `owner`), on the right for the owner and where `right` is given,
/// across the row where `wide` (a message the owner answers in it); `head` over what it says, `after` under it.
export function Bubble({ who, right = false, wide = false, ask, head, after, children }: {
  who: string;
  right?: boolean;
  wide?: boolean;
  /// The ask it is, which brings it into view.
  ask?: number;
  head: ReactNode;
  after?: ReactNode;
  children: ReactNode;
}) {
  const own = who === 'owner';
  const row = own ? 'talk-row talk-row-owner' : right ? 'talk-row talk-row-right' : wide ? 'talk-row talk-row-answers' : 'talk-row';
  return (
    <div className={row} data-ask={ask}>
      {own || right ? null : <Avatar who={who} />}
      <div className={`talk-bubble talk-bubble-${toneOf(who)}`}>
        <div className="talk-bubble-head">{head}</div>
        <div className="talk-bubble-body">{children}</div>
        {after}
      </div>
      {right && !own ? <Avatar who={who} /> : null}
    </div>
  );
}
