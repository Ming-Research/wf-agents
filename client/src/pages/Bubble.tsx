// A message of a talk, as a chat draws it, one part wherever the page shows agents and the owner speaking to each other
// (a conversation of the messages view, a helper's thread with the engineer on a work's page): an agent's in the box
// of what an agent handed back, filled and edged in its role's colour, the one the flow gives its line, beside its
// avatar; the owner's in the sheet of what is sent to an agent, on the right; and in a thread between two agents, the
// side across from the engineer on the right, its avatar after it. Its head (who, to whom, when, its round) over what
// it says, and under it what answers it there; under the owner's sheet, outside it, a line of the page's own may say
// where the words stand.

import { BugIcon, CodeReviewIcon, LightBulbIcon, ProjectRoadmapIcon, ToolsIcon } from '@primer/octicons-react';
import type { ReactNode } from 'react';

import { RunningMark } from '../components';
import { MarkdownLine } from '../markdown';
import { roleClass, useColours } from '../roles';

/// The mark of each line that speaks in a talk: the engineer's tools, the architect's plan, the advisor's light, the
/// review's and the adversarial testing's own; another line takes the engineer's.
const MARKS: Record<string, ReactNode> = {
  engineer: <ToolsIcon />,
  architect: <ProjectRoadmapIcon />,
  advisor: <LightBulbIcon />,
  review: <CodeReviewIcon />,
  adversarial: <BugIcon />,
};

/// An agent's avatar: its line's mark, white in a circle of its role's colour (`role`, the colour's class).
function Avatar({ who, role }: { who: string; role: string }) {
  return (
    <span className={`talk-avatar ${role}`} aria-hidden="true">
      {MARKS[who] ?? <ToolsIcon />}
    </span>
  );
}

/// One message: `who` says it (an agent's line, or `owner`), on the right for the owner and where `right` is given,
/// across the row where `wide` (a message the owner answers in it); `head` over what it says, `after` under it; for the
/// owner's, `under` a line of the page's own under the sheet.
export function Bubble({ who, right = false, wide = false, ask, head, after, under, children }: {
  who: string;
  right?: boolean;
  wide?: boolean;
  /// The ask it is, which brings it into view.
  ask?: number;
  head: ReactNode;
  after?: ReactNode;
  under?: ReactNode;
  children: ReactNode;
}) {
  const colours = useColours();
  const own = who === 'owner';
  const role = roleClass(colours, who);
  const row = own ? 'talk-row talk-row-owner' : right ? 'talk-row talk-row-right' : wide ? 'talk-row talk-row-answers' : 'talk-row';
  return (
    <div className={row} data-ask={ask}>
      {own || right ? null : <Avatar who={who} role={role} />}
      <div className={own ? 'talk-bubble talk-bubble-owner' : `talk-bubble talk-bubble-agent ${role}`}>
        <div className="talk-bubble-head">{head}</div>
        <div className="talk-bubble-body">{children}</div>
        {after}
      </div>
      {own ? under : null}
      {right && !own ? <Avatar who={who} role={role} /> : null}
    </div>
  );
}

/// A thing that runs and then ends, said as one bubble of one line (ours, one part for every such thing: a stage a work
/// entered, and next an architect's matter): `who` says it, in its bubble's look; over the line its head (what it is,
/// when it began, and links to where more of it is); the line itself, packed: while it goes on the running mark when a
/// round of it runs and what happens now (`state`), once over its result as a state's chip and why; after them the
/// gist of what was said last, cut where the line ends; the whole line said on hovering (`title`); and under the line
/// what more there is, each a fold that opens in place.
export function LiveBubble({ who, head, live, state, gist, title, children }: {
  who: string;
  head: ReactNode;
  live: boolean;
  state: ReactNode;
  gist: string | null;
  title: string;
  children?: ReactNode;
}) {
  return (
    <Bubble who={who} head={head}>
      <div className="live-line" title={title}>
        {live ? <RunningMark /> : null}
        <span className="live-line-state">{state}</span>
        {gist !== null ? (
          <span className="live-line-gist">
            <MarkdownLine text={gist} />
          </span>
        ) : null}
      </div>
      {children}
    </Bubble>
  );
}
