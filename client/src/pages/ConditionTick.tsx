// The tick on a box of the owner's words that sets a merge condition, one part in every box that takes it: the box at the
// bottom of a work's group, the small boxes in a bubble that answer an engineer's ask or question, and, in the one-to-one,
// the box at the bottom while it replies to a question about one item. Ticked, it adds one line, the condition, which
// holds the box's words until the owner writes in it, since words such as "OK" do not read alone; a line left empty keeps
// the words from being sent. What the box sends carries the line's sentence, and the engine records it with the words.

import { Checkbox, FormControl } from '@primer/react';

import { useDraft, type Draft } from '../drafts';
import { DraftInput } from '../parts';
import { t } from '../text';

/// What the kept draft of the tick holds while the box is ticked.
const TICKED = 'ticked';

export type ConditionTick = {
  /// Whether the box takes a tick at all where it stands.
  takes: boolean;
  ticked: boolean;
  /// The line of the condition, kept as every box's words are.
  line: Draft;
  /// Whether the box's words may go as they are: not ticked, or ticked with something on the line.
  ready: boolean;
  /// What goes with the words: the sentence, once ticked.
  sent: { condition?: string };
  set: (on: boolean) => void;
  /// Takes the tick and the line away, as a send that came through does.
  done: () => void;
};

/// The tick of the box whose draft is under `box`, with `words` the box's words: `takes` says whether the box takes a tick
/// where it stands. Whether it is ticked is kept as the words are, so that it stays with them when the box goes away.
export function useConditionTick(box: string, words: string, takes: boolean): ConditionTick {
  const flag = useDraft(`condition-ticked:${box}`);
  const line = useDraft(`condition:${box}`, words);
  const ticked = takes && flag.text === TICKED;
  const sentence = line.text.trim();
  const done = () => {
    flag.clear();
    line.clear();
  };
  return {
    takes,
    ticked,
    line,
    ready: !ticked || sentence !== '',
    sent: ticked ? { condition: sentence } : {},
    set: (on) => (on ? flag.setText(TICKED) : done()),
    done,
  };
}

/// The tick, and under it while ticked the line of the condition.
export function ConditionLine({ tick, disabled }: { tick: ConditionTick; disabled: boolean }) {
  if (!tick.takes) return null;
  return (
    <div className="condition-tick">
      <FormControl layout="horizontal" disabled={disabled}>
        <Checkbox checked={tick.ticked} onChange={(e) => tick.set(e.target.checked)} />
        <FormControl.Label>{t('condition_tick')}</FormControl.Label>
      </FormControl>
      {tick.ticked ? (
        <FormControl disabled={disabled}>
          <FormControl.Label>{t('condition_line')}</FormControl.Label>
          <DraftInput draft={tick.line} block validationStatus={tick.ready ? undefined : 'error'} />
          <FormControl.Caption>{t('condition_line_hint')}</FormControl.Caption>
        </FormControl>
      ) : null}
    </div>
  );
}
