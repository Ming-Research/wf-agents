// The engine's part of the bar, at its right: how the engine stands, in a dot and a few words; the switch of every
// work, "Pause all" while works go on and "Start all" while they are paused; the switch that keeps waiting sessions
// warm; and the slots in use. What the engine does not give yet comes from engine-mock.ts.

import { useCallback } from 'react';

import { useMockEngine } from './engine-mock';
import { Button, Switch } from './parts';
import type { Items, Now } from './works-api';
import { useChanges, useReading, useSource } from './works/source';

/// Whether a work waits for the owner: for an answer, for a test only the owner can make hold, at a limit, or
/// stopped by the engine or by a fault.
export function waitsForOwner(now: Now): boolean {
  if (now.kind === 'fault') return true;
  if (now.kind === 'paused') return now.by === 'engine';
  if (now.kind !== 'waits') return false;
  return now.on.some((w) => w.kind === 'limit' || w.who === 'owner');
}

/// The items read for the bar, and read again whenever something happened.
export function useBarItems(): { items: Items | null; failed: string | null } {
  const source = useSource();
  const load = useCallback(() => source.loadItems(), [source]);
  const { value, failed, read } = useReading(load);
  useChanges(read, read);
  return { items: value, failed };
}

export function Engine({ items, failed }: { items: Items | null; failed: string | null }) {
  const [mock, change] = useMockEngine();
  if (items === null) {
    return (
      <div className="engine">
        <span className="engine-state" title="The page cannot reach the engine; it reconnects by itself.">
          <span className={failed === null ? 'engine-dot engine-dot-paused' : 'engine-dot engine-dot-off'} />
          <span className="engine-words">{failed === null ? 'Reading' : 'Offline'}</span>
        </span>
      </div>
    );
  }
  const running = items.items.filter((i) => i.work.now.kind === 'round').length;
  const [dot, words] = !mock.paused
    ? ['run', running === 0 ? 'Running' : `Running: ${running} ${running === 1 ? 'round' : 'rounds'}`]
    : running > 0
      ? ['pausing', `Pausing: ${running} ${running === 1 ? 'round' : 'rounds'} finishing`]
      : ['paused', 'Paused'];
  return (
    <div className="engine">
      <span
        className="engine-state"
        title={mock.paused ? 'Every work is paused: no work goes on by itself and no new work starts.' : undefined}
      >
        <span className={`engine-dot engine-dot-${dot}`} />
        <span className="engine-words">{words}</span>
      </span>
      <Button
        title={
          mock.paused
            ? 'Every work goes on but those paused by themselves, and new works start on ready items as slots free.'
            : 'Every work stops at the end of the round it is in, and no new round or work starts.'
        }
        onClick={() => change({ paused: !mock.paused })}
      >
        {mock.paused ? 'Start all' : 'Pause all'}
      </Button>
      <span className="engine-warm">
        <span className="muted">Keep warm</span>
        <Switch<'on' | 'off'>
          label="Keep warm"
          value={mock.keepWarm ? 'on' : 'off'}
          onChange={(v) => change({ keepWarm: v === 'on' })}
          choices={[
            {
              value: 'on',
              name: 'On',
              title: 'A waiting session gets a keep-warm ping before its cache runs out, so that its next round reads the cache.',
            },
            {
              value: 'off',
              name: 'Off',
              title: 'No keep-warm ping goes out, and a round after an hour writes its cache again whole.',
            },
          ]}
        />
      </span>
      <span className="muted" title="Works with a round under way / the installation's slots.">
        Slots {running}/{mock.slots}
      </span>
    </div>
  );
}
