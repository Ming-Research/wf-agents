// The engine's part of the bar, at its right: how the engine stands, in a dot and a few words; the switch of every
// work, "Pause all" while works go on and "Start all" while they are paused; the switch that keeps waiting sessions
// warm; and the slots in use. All of them are the engine's own, and with the mocks shown (`mock-data.ts`) the made-up
// ones of engine-mock.ts stand in for them.

import { useCallback, useState } from 'react';

import { useMockEngine } from './engine-mock';
import { mocksShown } from './mock-data';
import { Button, Switch } from './parts';
import type { Items } from './works-api';
import { useStreamLost } from './changes';
import { useChanges, useItems, useReading, useSource } from './works/source';

/// The items read for the bar, and read again whenever something happened.
export type BarItems = { items: Items | null; failed: string | null };

/// The items the list of works reads too: read once for both (works/source.ts).
export function useBarItems(): BarItems {
  const { value, failed } = useItems();
  return { items: value, failed };
}

export function Engine({ items, failed }: { items: Items | null; failed: string | null }) {
  const [mock, change] = useMockEngine();
  const mocks = mocksShown();
  const source = useSource();
  const loadEngine = useCallback(
    () => (source.loadEngine === undefined ? Promise.resolve(null) : source.loadEngine()),
    [source],
  );
  const { value: real, read: readEngine } = useReading(loadEngine);
  useChanges(readEngine, readEngine);
  const [sending, setSending] = useState(false);
  const lost = useStreamLost();
  // The engine's own pause of everything and its keep-warm; with the mocks, the made-up ones.
  const paused = mocks ? mock.paused : real !== null && real.paused !== null;
  const warm = mocks ? mock.keepWarm : real !== null && real.keepWarm;
  const setWarm = (on: boolean) => {
    if (mocks) {
      change({ keepWarm: on });
      return;
    }
    if (sending || source.actOnEngine === undefined) return;
    setSending(true);
    void source
      .actOnEngine({ kind: 'keepWarm', on })
      .then(readEngine, readEngine)
      .finally(() => setSending(false));
  };
  const toggle = () => {
    if (mocks) {
      change({ paused: !mock.paused });
      return;
    }
    if (sending || source.actOnEngine === undefined) return;
    setSending(true);
    void source
      .actOnEngine({ kind: paused ? 'resume' : 'pause' })
      .then(readEngine, readEngine)
      .finally(() => setSending(false));
  };
  if (items === null || lost) {
    return (
      <div className="engine">
        <span
          className="engine-state"
          title="The page cannot reach the engine; it reconnects by itself. What it shows is from before."
        >
          <span className={failed === null && !lost ? 'engine-dot engine-dot-paused' : 'engine-dot engine-dot-off'} />
          <span className="engine-words">{failed === null && !lost ? 'Reading' : 'Offline'}</span>
        </span>
      </div>
    );
  }
  const running = items.items.filter((i) => i.work.now.kind === 'round').length;
  // Who holds every work is said here, where the pause of everything is: the engine's own, taken as it starts again on
  // works that were going on, is told apart from the owner's.
  const byEngine = !mocks && real?.paused?.by === 'engine';
  const [dot, words] = !paused
    ? ['run', running === 0 ? 'Running' : `Running: ${running} ${running === 1 ? 'round' : 'rounds'}`]
    : running > 0
      ? ['pausing', `Pausing: ${running} ${running === 1 ? 'round' : 'rounds'} finishing`]
      : [byEngine ? 'held' : 'paused', byEngine ? 'Paused by the engine' : 'Paused by you'];
  return (
    <div className="engine">
      <span
        className="engine-state"
        title={
          !paused
            ? undefined
            : byEngine
              ? 'The engine paused every work as it started again on works that were going on: no work goes on by itself until you start them.'
              : 'Every work is paused: no work goes on by itself and no new work starts.'
        }
      >
        <span className={`engine-dot engine-dot-${dot}`} />
        <span className="engine-words">{words}</span>
      </span>
      {(mocks || real !== null) && (
        <>
          <Button
            title={
              paused
                ? 'Every work goes on again but those paused by themselves.'
                : 'No work begins a round, except to answer a note, until every work is started again; a round under way finishes.'
            }
            onClick={toggle}
          >
            {paused ? 'Start all' : 'Pause all'}
          </Button>
          {(mocks || real !== null) && (
            <span className="engine-warm">
              <span className="muted">Keep warm</span>
              <Switch<'on' | 'off'>
                label="Keep warm"
                value={warm ? 'on' : 'off'}
                onChange={(v) => setWarm(v === 'on')}
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
          )}
          <span className="muted" title="Rounds of agents under way / how many may go on at once.">
            Slots {mocks || real === null ? `${running}/${mock.slots}` : `${real.taken}/${real.slots}`}
          </span>
        </>
      )}
    </div>
  );
}
