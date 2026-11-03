// The owner's hold on a project's new works, in the architect's title row, which is the one place that says what stops
// a project's new works from starting (client/ui-spec.md, "What stops new works"): the button that holds them, and,
// on each hold, the button that releases it. Each asks first, in a small form under the row: holding, why and what
// follows from it; releasing, what it lets happen. Holding is a rule on one project's new works, kept in the database,
// and is another thing from the top bar's Pause all, which stops every work and is not kept.

import { Button } from '@primer/react';
import { useState } from 'react';

import { releaseWords } from '../architect';
import { useDraft } from '../drafts';
import { postJson } from '../http';
import { SendForm } from '../parts';
import { t } from '../text';

/// What the form under the row is about: `hold` for holding new works, or the reason of the hold it releases.
export const HOLD = 'hold';

/// The path the owner's hold on a project's new works and its release are sent to.
const holdPath = (project: string): string => `/api/v1/projects/${encodeURIComponent(project)}/hold`;

/// The form that asks before the owner holds a project's new works or releases a hold on them: holding says what follows
/// (no new work starts, the running ones go on and merge, Pause all is another thing) and cannot be sent without the
/// words saying why; releasing says what it lets happen and is sent on the owner's confirmation. `onDone` closes it.
export function HoldAsk({ project, asking, onDone }: { project: string; asking: string; onDone: () => void }) {
  const draft = useDraft(`hold:${project}`);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (asking === HOLD) {
    return (
      <div className="talk-head-ask">
        <p className="talk-head-ask-words">{t('hold_start_asks', { project })}</p>
        <SendForm
          draft={draft}
          placeholder={t('hold_why')}
          rows={2}
          label={t('hold_start')}
          ready={draft.text.trim() !== ''}
          why={t('hold_needs_why')}
          onSend={() => draft.send(() => postJson(holdPath(project), { why: draft.text.trim() }), onDone)}
        />
        <Button size="small" variant="invisible" onClick={onDone}>
          {t('hold_cancel')}
        </Button>
      </div>
    );
  }
  const release = () => {
    setBusy(true);
    setError(null);
    postJson(`${holdPath(project)}/release`, { reason: asking }).then(
      () => {
        setBusy(false);
        onDone();
      },
      (x: unknown) => {
        setBusy(false);
        setError(x instanceof Error ? x.message : String(x));
      },
    );
  };
  return (
    <div className="talk-head-ask">
      <p className="talk-head-ask-words">{releaseWords(asking, project)}</p>
      <div className="talk-head-ask-buttons">
        <Button size="small" variant="primary" onClick={release} disabled={busy}>
          {t('hold_release')}
        </Button>
        <Button size="small" variant="invisible" onClick={onDone}>
          {t('hold_cancel')}
        </Button>
      </div>
      {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
    </div>
  );
}
