// What the console's marks and keys mean: a small mark at the tree's bottom left corner, the plainest question mark
// drawn as the page's signs are, which shows, while the pointer is over it or over what it shows, or the keys are on
// it, a panel with the legend of every mark the console's tree draws (`MARKS` in marks.tsx) and the keys the console
// takes (principle every-mark-is-explained).

import { useState } from 'react';

import { Label } from '../parts';
import { Legend, type MarkKey } from '../marks';

/// The marks the console's tree draws, in the order its legend lists them: a card by how its work stands, then a
/// card's tags and rows, then what joins the cards, and the map.
export const CONSOLE_MARKS: MarkKey[] = [
  'cardStopped',
  'cardRunning',
  'cardWaiting',
  'cardPaused',
  'cardAbove',
  'cardMerged',
  'cardCancelled',
  'cardDelivers',
  'cardElsewhere',
  'forYou',
  'waitsFor',
  'sentBack',
  'level',
  'cells',
  'lines',
  'age',
  'funnel',
  'folded',
  'litWay',
  'map',
];

/// The keys the console takes, each with what it does.
export const CONSOLE_KEYS: [string, string][] = [
  ['/', 'search'],
  ['e', 'fold or unfold the card under the pointer'],
  ['Esc', 'close'],
];

/// What the mark says it shows, on pointing at it and to a reader of the screen.
export const HELP_SAYS = "What the console's marks and keys mean";

export function ConsoleHelp() {
  const [shown, setShown] = useState(false);
  return (
    <div className="console-help" onPointerEnter={() => setShown(true)} onPointerLeave={() => setShown(false)}>
      {/* Always there, so that it fades in and out; hidden, it takes no pointer and is read by no reader. */}
      <div
        className={shown ? 'console-help-panel shown' : 'console-help-panel'}
        role="note"
        aria-label={HELP_SAYS}
        aria-hidden={!shown}
      >
        <Label>Marks</Label>
        <Legend marks={CONSOLE_MARKS} label="The console's marks" />
        <Label>Keys</Label>
        <ul className="legend" aria-label="The console's keys">
          {CONSOLE_KEYS.map(([key, does]) => (
            <li key={key}>
              <span className="mono console-help-key">{key}</span> {does}
            </li>
          ))}
        </ul>
      </div>
      <button
        type="button"
        className="button console-help-mark"
        aria-label={HELP_SAYS}
        aria-expanded={shown}
        title={HELP_SAYS}
        onFocus={() => setShown(true)}
        onBlur={() => setShown(false)}
      >
        <svg
          viewBox="0 0 12 12"
          width="12"
          height="12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.25"
          aria-hidden="true"
        >
          <circle cx="6" cy="6" r="4.75" />
          <path d="M4.6 4.7a1.45 1.45 0 1 1 2.1 1.3c-.45.25-.7.55-.7 1.1v.3" />
          <path d="M6 8.75v.01" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}
